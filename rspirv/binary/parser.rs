// Copyright 2016 Google Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use mr;
use grammar;
use spirv;

use std::{error, fmt, result, slice};
use super::{
    decoder,
    DecodeError,
    tracker::{Type, TypeTracker},
};

use grammar::CoreInstructionTable as GInstTable;
use grammar::OperandKind as GOpKind;
use grammar::OperandQuantifier as GOpCount;

use utils::version;

type GInstRef = &'static grammar::Instruction<'static>;

const WORD_NUM_BYTES: usize = 4;

/// Parser State.
///
/// Most of the error variants will retain the error location for both byte
/// offset (starting from 0) and instruction number (starting from 1).
#[derive(Debug)]
pub enum State {
    /// Parsing completed
    Complete,
    /// Consumer requested to stop parse
    ConsumerStopRequested,
    /// Consumer errored out with the given error
    ConsumerError(Box<error::Error>),
    /// Incomplete module header
    HeaderIncomplete(DecodeError),
    /// Incorrect module header
    HeaderIncorrect,
    /// Unsupported endianness
    EndiannessUnsupported,
    /// Zero instruction word count at (byte offset, inst number)
    WordCountZero(usize, usize),
    /// Unknown opcode at (byte offset, inst number, opcode)
    OpcodeUnknown(usize, usize, u16),
    /// Expected more operands (byte offset, inst number)
    OperandExpected(usize, usize),
    /// found redundant operands (byte offset, inst number)
    OperandExceeded(usize, usize),
    /// Errored out when decoding operand with the given error
    OperandError(DecodeError),
    /// Unsupported type (byte offset, inst number)
    TypeUnsupported(usize, usize),
    /// Incorrect SpecConstantOp Integer (byte offset, inst number)
    SpecConstantOpIntegerIncorrect(usize, usize),
}

impl From<DecodeError> for State {
    fn from(err: DecodeError) -> Self {
        State::OperandError(err)
    }
}

impl error::Error for State {
    fn description(&self) -> &str {
        match *self {
            State::Complete => "completed parsing",
            State::ConsumerStopRequested => "stop parsing requested by consumer",
            State::ConsumerError(_) => "consumer error",
            State::HeaderIncomplete(_) => "incomplete module header",
            State::HeaderIncorrect => "incorrect module header",
            State::EndiannessUnsupported => "unsupported endianness",
            State::WordCountZero(..) => "zero word count found",
            State::OpcodeUnknown(..) => "unknown opcode",
            State::OperandExpected(..) => "expected more operands",
            State::OperandExceeded(..) => "found extra operands",
            State::OperandError(_) => "operand decoding error",
            State::TypeUnsupported(..) => "unsupported type",
            State::SpecConstantOpIntegerIncorrect(..) => "incorrect SpecConstantOp Integer",
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            State::Complete => write!(f, "completed parsing"),
            State::ConsumerStopRequested => write!(f, "stop parsing requested by consumer"),
            State::ConsumerError(ref err) => write!(f, "consumer error: {}", err),
            State::HeaderIncomplete(ref err) => write!(f, "incomplete module header: {}", err),
            State::HeaderIncorrect => write!(f, "incorrect module header"),
            State::EndiannessUnsupported => write!(f, "unsupported endianness"),
            State::WordCountZero(offset, index) => {
                write!(f,
                       "zero word count found for instruction #{} at offset {}",
                       index,
                       offset)
            }
            State::OpcodeUnknown(offset, index, opcode) => {
                write!(f,
                       "unknown opcode ({}) for instruction #{} at offset {}",
                       opcode,
                       index,
                       offset)
            }
            State::OperandExpected(offset, index) => {
                write!(f,
                       "expected more operands for instruction #{} at offset \
                        {}",
                       index,
                       offset)
            }
            State::OperandExceeded(offset, index) => {
                write!(f,
                       "found extra operands for instruction #{} at offset {}",
                       index,
                       offset)
            }
            State::OperandError(ref err) => write!(f, "operand decoding error: {}", err),
            State::TypeUnsupported(offset, index) => {
                write!(f,
                       "unsupported type for instruction #{} at offset {}",
                       index,
                       offset)
            }
            State::SpecConstantOpIntegerIncorrect(offset, index) => {
                write!(f,
                       "incorrect SpecConstantOp number for instruction #{} at offset {}",
                       index,
                       offset)
            }
        }
    }
}

pub type Result<T> = result::Result<T, State>;

const HEADER_NUM_WORDS: usize = 5;

/// Orders consumer sent to the parser after each consuming call.
#[derive(Debug)]
pub enum Action {
    /// Continue the parsing
    Continue,
    /// Normally stop the parsing
    Stop,
    /// Error out with the given error
    Error(Box<error::Error>),
}

impl Action {
    fn consume(self) -> Result<()> {
        match self {
            Action::Continue => Ok(()),
            Action::Stop => Err(State::ConsumerStopRequested),
            Action::Error(err) => Err(State::ConsumerError(err)),
        }
    }
}

/// The binary consumer trait.
///
/// The parser will call `initialize` before parsing the SPIR-V binary and
/// `finalize` after successfully parsing the whle binary.
///
/// After successfully parsing the module header, `consume_header` will be
/// called. After successfully parsing an instruction, `consume_instruction`
/// will be called.
///
/// The consumer can use [`Action`](enum.ParseAction.html) to control the
/// parsing process.
pub trait Consumer {
    /// Intialize the consumer.
    fn initialize(&mut self) -> Action;
    /// Finalize the consumer.
    fn finalize(&mut self) -> Action;

    /// Consume the module header.
    fn consume_header(&mut self, module: mr::ModuleHeader) -> Action;
    /// Consume the given instruction.
    fn consume_instruction(&mut self, inst: mr::Instruction) -> Action;
}

/// Parses the given `binary` and consumes the module using the given
/// `consumer`.
pub fn parse_bytes<T: AsRef<[u8]>>(binary: T, consumer: &mut Consumer) -> Result<()> {
    Parser::new(binary.as_ref(), consumer).parse()
}

/// Parses the given `binary` and consumes the module using the given
/// `consumer`.
pub fn parse_words<T: AsRef<[u32]>>(binary: T, consumer: &mut Consumer) -> Result<()> {
    let len = binary.as_ref().len() * 4;
    let buf = unsafe { slice::from_raw_parts(binary.as_ref().as_ptr() as *const u8, len) };
    Parser::new(buf, consumer).parse()
}

/// The SPIR-V binary parser.
///
/// Takes in a vector of bytes and a consumer, this parser will invoke the
/// consume methods on the consumer for the module header and each
/// instruction parsed.
///
/// Different from the [`Decoder`](struct.Decoder.html),
/// this parser is high-level; it has knowlege of the SPIR-V grammar.
/// It will parse instructions according to SPIR-V grammar.
///
/// # Examples
///
/// ```
/// extern crate rspirv;
/// extern crate spirv_headers as spirv;
///
/// use spirv::{AddressingModel, MemoryModel};
/// use rspirv::binary::Parser;
/// use rspirv::mr::{Loader, Operand};
///
/// fn main() {
///     let bin = vec![
///         // Magic number.           Version number: 1.0.
///         0x03, 0x02, 0x23, 0x07,    0x00, 0x00, 0x01, 0x00,
///         // Generator number: 0.    Bound: 0.
///         0x00, 0x00, 0x00, 0x00,    0x00, 0x00, 0x00, 0x00,
///         // Reserved word: 0.
///         0x00, 0x00, 0x00, 0x00,
///         // OpMemoryModel.          Logical.
///         0x0e, 0x00, 0x03, 0x00,    0x00, 0x00, 0x00, 0x00,
///         // GLSL450.
///         0x01, 0x00, 0x00, 0x00];
///     let mut loader = Loader::new();  // You can use your own consumer here.
///     {
///         let p = Parser::new(&bin, &mut loader);
///         p.parse().unwrap();
///     }
///     let module = loader.module();
///
///     assert_eq!((1, 0), module.header.unwrap().version());
///     let m = module.memory_model.as_ref().unwrap();
///     assert_eq!(Operand::AddressingModel(AddressingModel::Logical),
///                m.operands[0]);
///     assert_eq!(Operand::MemoryModel(MemoryModel::GLSL450),
///                m.operands[1]);
/// }
/// ```
pub struct Parser<'c, 'd> {
    decoder: decoder::Decoder<'d>,
    consumer: &'c mut Consumer,
    type_tracker: TypeTracker,
    /// The index of the current instructions
    ///
    /// Starting from 1, 0 means invalid
    inst_index: usize,
}

impl<'c, 'd> Parser<'c, 'd> {
    /// Creates a new parser to parse the given `binary` and send the module
    /// header and instructions to the given `consumer`.
    pub fn new(binary: &'d [u8], consumer: &'c mut Consumer) -> Self {
        Parser {
            decoder: decoder::Decoder::new(binary),
            consumer: consumer,
            type_tracker: TypeTracker::new(),
            inst_index: 0,
        }
    }

    /// Does the parsing.
    pub fn parse(mut self) -> Result<()> {
        self.consumer.initialize().consume()?;
        let header = self.parse_header()?;
        self.consumer.consume_header(header).consume()?;

        loop {
            let result = self.parse_inst();
            match result {
                Ok(inst) => {
                    self.type_tracker.track(&inst);
                    self.consumer.consume_instruction(inst).consume()?;
                }
                Err(State::Complete) => break,
                Err(error) => return Err(error),
            };
        }
        self.consumer.finalize().consume()
    }

    fn split_into_word_count_and_opcode(word: spirv::Word) -> (u16, u16) {
        ((word >> 16) as u16, (word & 0xffff) as u16)
    }

    fn parse_header(&mut self) -> Result<mr::ModuleHeader> {
        match self.decoder.words(HEADER_NUM_WORDS) {
            Ok(words) => {
                if words[0] != spirv::MAGIC_NUMBER {
                    if words[0] == spirv::MAGIC_NUMBER.swap_bytes() {
                        return Err(State::EndiannessUnsupported);
                    } else {
                        return Err(State::HeaderIncorrect);
                    }
                }
                
                let mut header = mr::ModuleHeader::new(words[3]);
                let (major, minor) = version::create_version_from_word(words[1]);
                header.set_version(major, minor);

                Ok(header)
            }
            Err(err) => Err(State::HeaderIncomplete(err)),
        }
    }

    fn parse_inst(&mut self) -> Result<mr::Instruction> {
        self.inst_index += 1;
        if let Ok(word) = self.decoder.word() {
            let (wc, opcode) = Parser::split_into_word_count_and_opcode(word);
            if wc == 0 {
                return Err(State::WordCountZero(self.decoder.offset() - WORD_NUM_BYTES,
                                                self.inst_index));
            }
            if let Some(grammar) = GInstTable::lookup_opcode(opcode) {
                self.decoder.set_limit((wc - 1) as usize);
                let result = self.parse_operands(grammar);
                if !self.decoder.limit_reached() {
                    return Err(State::OperandExceeded(self.decoder.offset(), self.inst_index));
                }
                self.decoder.clear_limit();
                result
            } else {
                Err(State::OpcodeUnknown(self.decoder.offset() - WORD_NUM_BYTES,
                                         self.inst_index,
                                         opcode))
            }
        } else {
            Err(State::Complete)
        }
    }

    fn parse_literal(&mut self, type_id: spirv::Word) -> Result<mr::Operand> {
        let tracked_type = self.type_tracker.resolve(type_id);
        match tracked_type {
            Some(t) => {
                match t {
                    Type::Integer(size, _) => {
                        match size {
                            32 => Ok(mr::Operand::LiteralInt32(self.decoder.int32()?)),
                            64 => Ok(mr::Operand::LiteralInt64(self.decoder.int64()?)),
                            _ => {
                                Err(State::TypeUnsupported(self.decoder.offset(), self.inst_index))
                            }
                        }
                    }
                    Type::Float(size) => {
                        match size {
                            32 => {
                                Ok(mr::Operand::LiteralFloat32(self.decoder.float32()?))
                            }
                            64 => {
                                Ok(mr::Operand::LiteralFloat64(self.decoder.float64()?))
                            }
                            _ => {
                                Err(State::TypeUnsupported(self.decoder.offset(), self.inst_index))
                            }
                        }
                    }
                }
            }
            // Treat as a normal SPIR-V word if we don't know the type.
            // TODO: find a better way to handle this.
            None => Ok(mr::Operand::LiteralInt32(self.decoder.int32()?)),
        }
    }

    fn parse_spec_constant_op(&mut self) -> Result<Vec<mr::Operand>> {
        let mut operands = vec![];

        let number = self.decoder.int32()?;
        if let Some(g) = GInstTable::lookup_opcode(number as u16) {
            // TODO: check whether this opcode is allowed here.
            operands.push(mr::Operand::LiteralSpecConstantOpInteger(g.opcode));
            // We need id parameters to this SpecConstantOp.
            for operand in g.operands {
                if operand.kind == GOpKind::IdRef {
                    operands.push(mr::Operand::IdRef(self.decoder.id()?))
                }
            }
            Ok(operands)
        } else {
            Err(State::SpecConstantOpIntegerIncorrect(self.decoder.offset(), self.inst_index))
        }
    }

    fn parse_operands(&mut self, grammar: GInstRef) -> Result<mr::Instruction> {
        let mut rtype = None;
        let mut rid = None;
        let mut coperands = vec![]; // concrete operands

        let mut loperand_index: usize = 0; // logical operand index
        while loperand_index < grammar.operands.len() {
            let loperand = &grammar.operands[loperand_index];
            let has_more_coperands = !self.decoder.limit_reached();
            if has_more_coperands {
                match loperand.kind {
                    GOpKind::IdResultType => rtype = Some(self.decoder.id()?),
                    GOpKind::IdResult => rid = Some(self.decoder.id()?),
                    GOpKind::LiteralContextDependentNumber => {
                        // Only constant defining instructions use this kind.
                        // If it is not true, that means the grammar is wrong
                        // or has changed.
                        assert!(grammar.opcode == spirv::Op::Constant ||
                                grammar.opcode == spirv::Op::SpecConstant);
                        let id = rtype.expect("internal error: \
                            should already decoded result type id before context dependent number");
                        coperands.push(self.parse_literal(id)?)
                    }
                    GOpKind::LiteralSpecConstantOpInteger => {
                        coperands.append(&mut self.parse_spec_constant_op()?)
                    }
                    _ => coperands.append(&mut self.parse_operand(loperand.kind)?),
                }
                match loperand.quantifier {
                    GOpCount::One | GOpCount::ZeroOrOne => loperand_index += 1,
                    GOpCount::ZeroOrMore => continue,
                }
            } else {
                // We still have logical operands to match but no no more words.
                match loperand.quantifier {
                    GOpCount::One => {
                        return Err(State::OperandExpected(self.decoder.offset(), self.inst_index))
                    }
                    GOpCount::ZeroOrOne | GOpCount::ZeroOrMore => break,
                }
            }
        }
        Ok(mr::Instruction::new(grammar.opcode, rtype, rid, coperands))
    }
}

include!("autogen_parse_operand.rs");

#[cfg(test)]
mod tests {
    use mr;
    use spirv;

    use binary::DecodeError;
    use std::{error, fmt};
    use super::{Action, Consumer, parse_words, Parser, State, WORD_NUM_BYTES};

    use utils::num::{f32_to_bytes, f64_to_bytes};

    // TODO: It's unfortunate that we have these numbers directly coded here
    // and repeat them in the following tests. Should have a better way.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    static ZERO_BOUND_HEADER: &'static [u8] = &[
        // Magic number.           Version number: 1.0.
        0x03, 0x02, 0x23, 0x07,    0x00, 0x00, 0x01, 0x00,
        // Generator number: 0.    Bound: 0.
        0x00, 0x00, 0x00, 0x00,    0x00, 0x00, 0x00, 0x00,
        // Reserved word: 0.
        0x00, 0x00, 0x00, 0x00];

    struct RetainingConsumer {
        pub header: Option<mr::ModuleHeader>,
        pub insts: Vec<mr::Instruction>,
    }
    impl RetainingConsumer {
        fn new() -> RetainingConsumer {
            RetainingConsumer {
                header: None,
                insts: vec![],
            }
        }
    }
    impl Consumer for RetainingConsumer {
        fn initialize(&mut self) -> Action {
            Action::Continue
        }
        fn finalize(&mut self) -> Action {
            Action::Continue
        }

        fn consume_header(&mut self, header: mr::ModuleHeader) -> Action {
            self.header = Some(header);
            Action::Continue
        }
        fn consume_instruction(&mut self, inst: mr::Instruction) -> Action {
            self.insts.push(inst);
            Action::Continue
        }
    }

    // TODO: Should put this function and its duplicate in the decoder in
    // a utility module.
    fn w2b(word: spirv::Word) -> Vec<u8> {
        (0..WORD_NUM_BYTES)
            .map(|i| ((word >> (8 * i)) & 0xff) as u8)
            .collect()
    }

    /// A simple module builder for testing purpose.
    struct ModuleBuilder {
        insts: Vec<u8>,
    }
    impl ModuleBuilder {
        fn new() -> ModuleBuilder {
            ModuleBuilder { insts: ZERO_BOUND_HEADER.to_vec() }
        }

        /// Appends an instruction with the given `opcode` and `operands` into
        /// the module.
        fn inst(&mut self, opcode: spirv::Op, operands: Vec<u32>) {
            let count: u32 = operands.len() as u32 + 1;
            self.insts.append(&mut w2b((count << 16) | (opcode as u32)));
            for o in operands {
                self.insts.append(&mut w2b(o));
            }
        }

        /// Returns the module being constructed.
        fn get(&self) -> &[u8] {
            &self.insts
        }
    }

    #[test]
    fn test_module_builder() {
        let mut b = ModuleBuilder::new();
        // OpNop
        b.inst(spirv::Op::Nop, vec![]);
        // OpCapability Int16
        b.inst(spirv::Op::Capability, vec![22]);
        // OpMemoryModel Logical GLSL450
        b.inst(spirv::Op::MemoryModel, vec![0, 1]);
        let mut module = ZERO_BOUND_HEADER.to_vec();
        module.append(&mut vec![0x00, 0x00, 0x01, 0x00]); // OpNop
        module.append(&mut vec![0x11, 0x00, 0x02, 0x00]); // OpCapability
        module.append(&mut vec![0x16, 0x00, 0x00, 0x00]); // Int16
        module.append(&mut vec![0x0e, 0x00, 0x03, 0x00]); // OpMemoryModel
        module.append(&mut vec![0x00, 0x00, 0x00, 0x00]); // Logical
        module.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // GLSL450
        assert_eq!(module, b.get());
    }

    #[test]
    fn test_parsing_empty_binary() {
        let v = vec![];
        let mut c = RetainingConsumer::new();
        let p = Parser::new(&v, &mut c);
        assert_matches!(p.parse(),
                        Err(State::HeaderIncomplete(DecodeError::StreamExpected(0))));
    }

    #[test]
    fn test_parsing_incomplete_header() {
        let v = vec![0x03, 0x02, 0x23, 0x07];
        let mut c = RetainingConsumer::new();
        let p = Parser::new(&v, &mut c);
        assert_matches!(p.parse(),
                        Err(State::HeaderIncomplete(DecodeError::StreamExpected(4))));
    }

    #[test]
    fn test_parsing_unsupported_endianness() {
        let mut module = ZERO_BOUND_HEADER.to_vec();
        module.as_mut_slice().swap(0, 3);
        module.as_mut_slice().swap(1, 2);
        let mut c = RetainingConsumer::new();
        let p = Parser::new(&module, &mut c);
        assert_matches!(p.parse(), Err(State::EndiannessUnsupported));
    }

    #[test]
    fn test_parsing_wrong_magic_number() {
        let mut module = ZERO_BOUND_HEADER.to_vec();
        module[0] = 0x00;
        let mut c = RetainingConsumer::new();
        let p = Parser::new(&module, &mut c);
        assert_matches!(p.parse(), Err(State::HeaderIncorrect));
    }

    #[test]
    fn test_parsing_complete_header() {
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(ZERO_BOUND_HEADER, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        let mut header = mr::ModuleHeader::new(0);
        header.set_version(1, 0);
        assert_eq!(Some(header), c.header);
    }

    #[test]
    fn test_parsing_one_inst() {
        let mut c = RetainingConsumer::new();
        {
            let mut b = ModuleBuilder::new();
            // OpMemoryModel Logical GLSL450
            b.inst(spirv::Op::MemoryModel, vec![0, 1]);
            let p = Parser::new(b.get(), &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("MemoryModel", inst.class.opname);
        assert_eq!(None, inst.result_type);
        assert_eq!(None, inst.result_id);
        assert_eq!(vec![mr::Operand::AddressingModel(spirv::AddressingModel::Logical),
                        mr::Operand::MemoryModel(spirv::MemoryModel::GLSL450)],
                   inst.operands);
    }

    #[test]
    fn test_parsing_zero_word_count() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x00, 0x00, 0x00, 0x00]); // OpNop with word count 0
        let mut c = RetainingConsumer::new();
        let p = Parser::new(&v, &mut c);
        // The first instruction starts at byte offset 20.
        assert_matches!(p.parse(), Err(State::WordCountZero(20, 1)));
    }

    #[test]
    fn test_parsing_extra_operand() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x00, 0x00, 0x01, 0x00]); // OpNop with word count 1
        v.append(&mut vec![0x00, 0x00, 0x02, 0x00]); // OpNop with word count 2
        v.append(&mut vec![0x00, 0x00, 0x00, 0x00]); // A bogus operand
        let mut c = RetainingConsumer::new();
        let p = Parser::new(&v, &mut c);
        // The bogus operand to the second OpNop instruction starts at
        // byte offset (20 + 4 + 4).
        assert_matches!(p.parse(), Err(State::OperandExceeded(28, 2)));
    }

    #[test]
    fn test_parsing_missing_operand() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x00, 0x00, 0x01, 0x00]); // OpNop with word count 1
        v.append(&mut vec![0x0e, 0x00, 0x03, 0x00]); // OpMemoryModel
        v.append(&mut vec![0x00, 0x00, 0x00, 0x00]); // Logical
        let mut c = RetainingConsumer::new();
        let p = Parser::new(&v, &mut c);
        // The missing operand to the OpMemoryModel instruction starts at
        // byte offset (20 + 4 + 4 + 4).
        assert_matches!(p.parse(),
                        Err(State::OperandError(DecodeError::StreamExpected(32))));
    }

    #[test]
    fn test_parsing_operand_parameters() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x47, 0x00, 0x04, 0x00]); // OpDecorate
        v.append(&mut vec![0x05, 0x00, 0x00, 0x00]); // id 5
        v.append(&mut vec![0x0b, 0x00, 0x00, 0x00]); // BuiltIn
        v.append(&mut vec![0x06, 0x00, 0x00, 0x00]); // InstanceId
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("Decorate", inst.class.opname);
        assert_eq!(None, inst.result_type);
        assert_eq!(None, inst.result_id);
        assert_eq!(vec![mr::Operand::IdRef(5),
                        mr::Operand::Decoration(spirv::Decoration::BuiltIn),
                        mr::Operand::BuiltIn(spirv::BuiltIn::InstanceId)],
                   inst.operands);
    }

    #[test]
    fn test_parsing_missing_operand_parameters() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x47, 0x00, 0x03, 0x00]); // OpDecorate
        v.append(&mut vec![0x05, 0x00, 0x00, 0x00]); // id 5
        v.append(&mut vec![0x0b, 0x00, 0x00, 0x00]); // BuiltIn
        let mut c = RetainingConsumer::new();
        let p = Parser::new(&v, &mut c);
        assert_matches!(p.parse(),
                        Err(State::OperandError(DecodeError::StreamExpected(32))));
    }

    #[test]
    fn test_parsing_with_all_optional_operands() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x03, 0x00, 0x05, 0x00]); // OpSource
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // GLSL
        v.append(&mut vec![0xc2, 0x01, 0x00, 0x00]); // 450 (0x1c2)
        v.append(&mut vec![0x06, 0x00, 0x00, 0x00]); // File id
        v.append(&mut b"wow".to_vec());              // Source
        v.push(0x00);                                // EOS
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("Source", inst.class.opname);
        assert_eq!(None, inst.result_type);
        assert_eq!(None, inst.result_id);
        assert_eq!(vec![mr::Operand::SourceLanguage(spirv::SourceLanguage::GLSL),
                        mr::Operand::LiteralInt32(450),
                        mr::Operand::IdRef(6),
                        mr::Operand::from("wow")],
                   inst.operands);
    }

    #[test]
    fn test_parsing_missing_one_optional_operand() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x03, 0x00, 0x04, 0x00]); // OpSource
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // GLSL
        v.append(&mut vec![0xc2, 0x01, 0x00, 0x00]); // 450 (0x1c2)
        v.append(&mut vec![0x06, 0x00, 0x00, 0x00]); // File id
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("Source", inst.class.opname);
        assert_eq!(None, inst.result_type);
        assert_eq!(None, inst.result_id);
        assert_eq!(vec![mr::Operand::SourceLanguage(spirv::SourceLanguage::GLSL),
                        mr::Operand::LiteralInt32(450),
                        mr::Operand::IdRef(6)],
                   inst.operands);
    }

    #[test]
    fn test_parsing_missing_two_optional_operands() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x03, 0x00, 0x03, 0x00]); // OpSource
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // GLSL
        v.append(&mut vec![0xc2, 0x01, 0x00, 0x00]); // 450 (0x1c2)
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("Source", inst.class.opname);
        assert_eq!(None, inst.result_type);
        assert_eq!(None, inst.result_id);
        assert_eq!(vec![mr::Operand::SourceLanguage(spirv::SourceLanguage::GLSL),
                        mr::Operand::LiteralInt32(450)],
                   inst.operands);
    }

    #[derive(Debug)]
    struct ErrorString(&'static str);
    impl error::Error for ErrorString {
        fn description(&self) -> &str {
            "consumer error"
        }
    }
    impl fmt::Display for ErrorString {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let &ErrorString(ref s) = self;
            write!(f, "{}", s)
        }
    }

    struct InitializeErrorConsumer;
    impl Consumer for InitializeErrorConsumer {
        fn initialize(&mut self) -> Action {
            Action::Error(Box::new(ErrorString("init error")))
        }
        fn finalize(&mut self) -> Action {
            Action::Continue
        }
        fn consume_header(&mut self, _: mr::ModuleHeader) -> Action {
            Action::Continue
        }
        fn consume_instruction(&mut self, _: mr::Instruction) -> Action {
            Action::Continue
        }
    }

    #[test]
    fn test_consumer_initialize_error() {
        let v = vec![];
        let mut c = InitializeErrorConsumer {};
        let p = Parser::new(&v, &mut c);
        let ret = p.parse();
        assert_matches!(ret, Err(State::ConsumerError(_)));
        if let Err(State::ConsumerError(err)) = ret {
            assert_eq!("consumer error", err.description());
            assert_eq!("init error", format!("{}", err));
        } else {
            assert!(false);
        }
    }

    struct FinalizeErrorConsumer;
    impl Consumer for FinalizeErrorConsumer {
        fn initialize(&mut self) -> Action {
            Action::Continue
        }
        fn finalize(&mut self) -> Action {
            Action::Error(Box::new(ErrorString("fin error")))
        }
        fn consume_header(&mut self, _: mr::ModuleHeader) -> Action {
            Action::Continue
        }
        fn consume_instruction(&mut self, _: mr::Instruction) -> Action {
            Action::Continue
        }
    }

    #[test]
    fn test_consumer_finalize_error() {
        let mut c = FinalizeErrorConsumer {};
        let p = Parser::new(ZERO_BOUND_HEADER, &mut c);
        let ret = p.parse();
        assert_matches!(ret, Err(State::ConsumerError(_)));
        if let Err(State::ConsumerError(err)) = ret {
            assert_eq!("consumer error", err.description());
            assert_eq!("fin error", format!("{}", err));
        } else {
            assert!(false);
        }
    }

    struct ParseHeaderErrorConsumer;
    impl Consumer for ParseHeaderErrorConsumer {
        fn initialize(&mut self) -> Action {
            Action::Continue
        }
        fn finalize(&mut self) -> Action {
            Action::Continue
        }
        fn consume_header(&mut self, _: mr::ModuleHeader) -> Action {
            Action::Error(Box::new(ErrorString("parse header error")))
        }
        fn consume_instruction(&mut self, _: mr::Instruction) -> Action {
            Action::Continue
        }
    }

    #[test]
    fn test_consumer_parse_header_error() {
        let mut c = ParseHeaderErrorConsumer {};
        let p = Parser::new(ZERO_BOUND_HEADER, &mut c);
        let ret = p.parse();
        assert_matches!(ret, Err(State::ConsumerError(_)));
        if let Err(State::ConsumerError(err)) = ret {
            assert_eq!("consumer error", err.description());
            assert_eq!("parse header error", format!("{}", err));
        } else {
            assert!(false);
        }
    }

    struct ParseInstErrorConsumer;
    impl Consumer for ParseInstErrorConsumer {
        fn initialize(&mut self) -> Action {
            Action::Continue
        }
        fn finalize(&mut self) -> Action {
            Action::Continue
        }
        fn consume_header(&mut self, _: mr::ModuleHeader) -> Action {
            Action::Continue
        }
        fn consume_instruction(&mut self, _: mr::Instruction) -> Action {
            Action::Error(Box::new(ErrorString("parse inst error")))
        }
    }

    #[test]
    fn test_consumer_parse_inst_error() {
        let mut b = ModuleBuilder::new();
        b.inst(spirv::Op::Nop, vec![]);
        let mut c = ParseInstErrorConsumer {};
        let p = Parser::new(b.get(), &mut c);
        let ret = p.parse();
        assert_matches!(ret, Err(State::ConsumerError(_)));
        if let Err(State::ConsumerError(err)) = ret {
            assert_eq!("consumer error", err.description());
            assert_eq!("parse inst error", format!("{}", err));
        } else {
            assert!(false);
        }
    }

    #[test]
    fn test_parsing_int32() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x15, 0x00, 0x04, 0x00]); // OpTypeInt
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // result id: 1
        v.append(&mut vec![0x20, 0x00, 0x00, 0x00]); // 32
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // 1 (signed)

        v.append(&mut vec![0x2b, 0x00, 0x04, 0x00]); // OpConstant
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // result type: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // result id: 2
        v.append(&mut vec![0x12, 0x34, 0x56, 0x78]);
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(2, c.insts.len());
        let inst = &c.insts[1];
        assert_eq!("Constant", inst.class.opname);
        assert_eq!(Some(1), inst.result_type);
        assert_eq!(Some(2), inst.result_id);
        assert_eq!(vec![mr::Operand::LiteralInt32(0x78563412)], inst.operands);
    }

    #[test]
    fn test_parsing_int64() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x15, 0x00, 0x04, 0x00]); // OpTypeInt
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // result id: 1
        v.append(&mut vec![0x40, 0x00, 0x00, 0x00]); // 64
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // 1 (signed)

        v.append(&mut vec![0x2b, 0x00, 0x05, 0x00]); // OpConstant
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // result type: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // result id: 2
        v.append(&mut vec![0x12, 0x34, 0x56, 0x78]);
        v.append(&mut vec![0x90, 0xab, 0xcd, 0xef]);
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(2, c.insts.len());
        let inst = &c.insts[1];
        assert_eq!("Constant", inst.class.opname);
        assert_eq!(Some(1), inst.result_type);
        assert_eq!(Some(2), inst.result_id);
        assert_eq!(vec![mr::Operand::LiteralInt64(0xefcdab9078563412)],
                   inst.operands);
    }

    #[test]
    fn test_parsing_float32() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x16, 0x00, 0x03, 0x00]); // OpTypeFloat
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // result id: 1
        v.append(&mut vec![0x20, 0x00, 0x00, 0x00]); // 32

        v.append(&mut vec![0x2b, 0x00, 0x04, 0x00]); // OpConstant
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // result type: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // result id: 2
        v.append(&mut f32_to_bytes(42.42));
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(2, c.insts.len());
        let inst = &c.insts[1];
        assert_eq!("Constant", inst.class.opname);
        assert_eq!(Some(1), inst.result_type);
        assert_eq!(Some(2), inst.result_id);
        assert_eq!(vec![mr::Operand::LiteralFloat32(42.42)], inst.operands);
    }

    #[test]
    fn test_parsing_float64() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x16, 0x00, 0x03, 0x00]); // OpTypeFloat
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // result id: 1
        v.append(&mut vec![0x40, 0x00, 0x00, 0x00]); // 64

        v.append(&mut vec![0x2b, 0x00, 0x05, 0x00]); // OpConstant
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // result type: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // result id: 2
        v.append(&mut f64_to_bytes(-12.34));
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(2, c.insts.len());
        let inst = &c.insts[1];
        assert_eq!("Constant", inst.class.opname);
        assert_eq!(Some(1), inst.result_type);
        assert_eq!(Some(2), inst.result_id);
        assert_eq!(vec![mr::Operand::LiteralFloat64(-12.34)], inst.operands);
    }

    #[test]
    fn test_parsing_spec_constant_op() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x34, 0x00, 0x05, 0x00]); // OpTypeFloat
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // result type: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // result id: 2
        v.append(&mut vec![0x7e, 0x00, 0x00, 0x00]); // OpSNegate
        v.append(&mut vec![0x03, 0x00, 0x00, 0x00]); // id ref: 3
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("SpecConstantOp", inst.class.opname);
        assert_eq!(Some(1), inst.result_type);
        assert_eq!(Some(2), inst.result_id);
        assert_eq!(vec![mr::Operand::LiteralSpecConstantOpInteger(spirv::Op::SNegate),
                        mr::Operand::IdRef(3)],
                   inst.operands);
    }

    #[test]
    fn test_parsing_spec_constant_op_missing_parameter() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x34, 0x00, 0x05, 0x00]); // OpTypeFloat
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // result type: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // result id: 2
        v.append(&mut vec![0x80, 0x00, 0x00, 0x00]); // OpIAdd
        v.append(&mut vec![0x03, 0x00, 0x00, 0x00]); // id ref: 3
        let mut c = RetainingConsumer::new();
        let p = Parser::new(&v, &mut c);
        assert_matches!(p.parse(),
                        // The header has 5 words, the above instruction has 5 words,
                        // so in total 40 bytes.
                        Err(State::OperandError(DecodeError::LimitReached(40))));
    }

    #[test]
    fn test_parsing_bitmasks_requiring_params_no_mem_access() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x3e, 0x00, 0x03, 0x00]); // OpStore
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // pointer: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // object: 2
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("Store", inst.class.opname);
        assert_eq!(None, inst.result_type);
        assert_eq!(None, inst.result_id);
        assert_eq!(vec![mr::Operand::IdRef(1), mr::Operand::IdRef(2)],
                   inst.operands);
    }
    #[test]
    fn test_parsing_bitmasks_requiring_params_mem_access_no_param() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x3e, 0x00, 0x04, 0x00]); // OpStore
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // pointer: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // object: 2
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // Volatile
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("Store", inst.class.opname);
        assert_eq!(None, inst.result_type);
        assert_eq!(None, inst.result_id);
        assert_eq!(vec![mr::Operand::IdRef(1),
                        mr::Operand::IdRef(2),
                        mr::Operand::MemoryAccess(spirv::MemoryAccess::VOLATILE)],
                   inst.operands);
    }
    #[test]
    fn test_parsing_bitmasks_requiring_params_mem_access_with_param() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x3e, 0x00, 0x05, 0x00]); // OpStore
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // pointer: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // object: 2
        v.append(&mut vec![0x03, 0x00, 0x00, 0x00]); // Volatile & Aligned
        v.append(&mut vec![0x04, 0x00, 0x00, 0x00]); // alignment
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("Store", inst.class.opname);
        assert_eq!(None, inst.result_type);
        assert_eq!(None, inst.result_id);
        assert_eq!(vec![mr::Operand::IdRef(1),
                        mr::Operand::IdRef(2),
                        mr::Operand::MemoryAccess(spirv::MemoryAccess::from_bits(3).unwrap()),
                        mr::Operand::LiteralInt32(4)],
                   inst.operands);
    }
    #[test]
    fn test_parsing_bitmasks_requiring_params_mem_access_missing_param() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x3e, 0x00, 0x04, 0x00]); // OpStore
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // pointer: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // object: 2
        v.append(&mut vec![0x03, 0x00, 0x00, 0x00]); // Volatile & Aligned
        let mut c = RetainingConsumer::new();
        let p = Parser::new(&v, &mut c);
        assert_matches!(p.parse(),
                        // The header has 5 words, the above instruction has 4 words,
                        // so in total 36 bytes.
                        Err(State::OperandError(DecodeError::LimitReached(36))));
    }
    #[test]
    fn test_parsing_bitmasks_requiring_params_img_operands_param_order() {
        let mut v = ZERO_BOUND_HEADER.to_vec();
        v.append(&mut vec![0x63, 0x00, 0x08, 0x00]); // OpStore
        v.append(&mut vec![0x01, 0x00, 0x00, 0x00]); // image: 1
        v.append(&mut vec![0x02, 0x00, 0x00, 0x00]); // coordinate: 2
        v.append(&mut vec![0x03, 0x00, 0x00, 0x00]); // texel: 3
        v.append(&mut vec![0x05, 0x00, 0x00, 0x00]); // Bias & GRAD
        v.append(&mut vec![0xaa, 0x00, 0x00, 0x00]); // bias
        v.append(&mut vec![0xbb, 0x00, 0x00, 0x00]); // dx
        v.append(&mut vec![0xcc, 0x00, 0x00, 0x00]); // dy
        let mut c = RetainingConsumer::new();
        {
            let p = Parser::new(&v, &mut c);
            assert_matches!(p.parse(), Ok(()));
        }
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("ImageWrite", inst.class.opname);
        assert_eq!(None, inst.result_type);
        assert_eq!(None, inst.result_id);
        assert_eq!(vec![mr::Operand::IdRef(1),
                        mr::Operand::IdRef(2),
                        mr::Operand::IdRef(3),
                        mr::Operand::ImageOperands(spirv::ImageOperands::from_bits(5).unwrap()),
                        mr::Operand::IdRef(0xaa),
                        mr::Operand::IdRef(0xbb),
                        mr::Operand::IdRef(0xcc)],
                   inst.operands);
    }

    #[test]
    fn test_parse_words() {
        let words = vec![0x07230203, 0x01000000, 0, 0, 0, 0x00020011, 0x00000016];
        let mut c = RetainingConsumer::new();
        assert_matches!(parse_words(&words, &mut c), Ok(()));
        assert_eq!(1, c.insts.len());
        let inst = &c.insts[0];
        assert_eq!("Capability", inst.class.opname);
        assert_eq!(None, inst.result_type);
        assert_eq!(None, inst.result_id);
        assert_eq!(vec![mr::Operand::Capability(spirv::Capability::Int16)],
                   inst.operands);
    }
}
