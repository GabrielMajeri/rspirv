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

// AUTOMATICALLY GENERATED from the SPIR-V JSON grammar:
//   external/spirv.core.grammar.json.
// DO NOT MODIFY!

impl<'c, 'd> Parser<'c, 'd> {
    fn parse_operand(&mut self, kind: GOpKind) -> Result<Vec<mr::Operand>> {
        Ok(match kind {
            GOpKind::FPFastMathMode => vec![mr::Operand::FPFastMathMode(self.decoder.fpfast_math_mode()?)],
            GOpKind::SelectionControl => vec![mr::Operand::SelectionControl(self.decoder.selection_control()?)],
            GOpKind::FunctionControl => vec![mr::Operand::FunctionControl(self.decoder.function_control()?)],
            GOpKind::MemorySemantics => vec![mr::Operand::MemorySemantics(self.decoder.memory_semantics()?)],
            GOpKind::KernelProfilingInfo => vec![mr::Operand::KernelProfilingInfo(self.decoder.kernel_profiling_info()?)],
            GOpKind::SourceLanguage => vec![mr::Operand::SourceLanguage(self.decoder.source_language()?)],
            GOpKind::ExecutionModel => vec![mr::Operand::ExecutionModel(self.decoder.execution_model()?)],
            GOpKind::AddressingModel => vec![mr::Operand::AddressingModel(self.decoder.addressing_model()?)],
            GOpKind::MemoryModel => vec![mr::Operand::MemoryModel(self.decoder.memory_model()?)],
            GOpKind::StorageClass => vec![mr::Operand::StorageClass(self.decoder.storage_class()?)],
            GOpKind::Dim => vec![mr::Operand::Dim(self.decoder.dim()?)],
            GOpKind::SamplerAddressingMode => vec![mr::Operand::SamplerAddressingMode(self.decoder.sampler_addressing_mode()?)],
            GOpKind::SamplerFilterMode => vec![mr::Operand::SamplerFilterMode(self.decoder.sampler_filter_mode()?)],
            GOpKind::ImageFormat => vec![mr::Operand::ImageFormat(self.decoder.image_format()?)],
            GOpKind::ImageChannelOrder => vec![mr::Operand::ImageChannelOrder(self.decoder.image_channel_order()?)],
            GOpKind::ImageChannelDataType => vec![mr::Operand::ImageChannelDataType(self.decoder.image_channel_data_type()?)],
            GOpKind::FPRoundingMode => vec![mr::Operand::FPRoundingMode(self.decoder.fprounding_mode()?)],
            GOpKind::LinkageType => vec![mr::Operand::LinkageType(self.decoder.linkage_type()?)],
            GOpKind::AccessQualifier => vec![mr::Operand::AccessQualifier(self.decoder.access_qualifier()?)],
            GOpKind::FunctionParameterAttribute => vec![mr::Operand::FunctionParameterAttribute(self.decoder.function_parameter_attribute()?)],
            GOpKind::BuiltIn => vec![mr::Operand::BuiltIn(self.decoder.built_in()?)],
            GOpKind::Scope => vec![mr::Operand::Scope(self.decoder.scope()?)],
            GOpKind::GroupOperation => vec![mr::Operand::GroupOperation(self.decoder.group_operation()?)],
            GOpKind::KernelEnqueueFlags => vec![mr::Operand::KernelEnqueueFlags(self.decoder.kernel_enqueue_flags()?)],
            GOpKind::Capability => vec![mr::Operand::Capability(self.decoder.capability()?)],
            GOpKind::IdMemorySemantics => vec![mr::Operand::IdMemorySemantics(self.decoder.id()?)],
            GOpKind::IdScope => vec![mr::Operand::IdScope(self.decoder.id()?)],
            GOpKind::IdRef => vec![mr::Operand::IdRef(self.decoder.id()?)],
            GOpKind::LiteralInteger => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            GOpKind::LiteralString => vec![mr::Operand::LiteralString(self.decoder.string()?)],
            GOpKind::LiteralExtInstInteger => vec![mr::Operand::LiteralExtInstInteger(self.decoder.ext_inst_integer()?)],
            GOpKind::PairLiteralIntegerIdRef => {
                vec![mr::Operand::LiteralInt32(self.decoder.int32()?), mr::Operand::IdRef(self.decoder.id()?)]
            }
            GOpKind::PairIdRefLiteralInteger => {
                vec![mr::Operand::IdRef(self.decoder.id()?), mr::Operand::LiteralInt32(self.decoder.int32()?)]
            }
            GOpKind::PairIdRefIdRef => {
                vec![mr::Operand::IdRef(self.decoder.id()?), mr::Operand::IdRef(self.decoder.id()?)]
            }
            GOpKind::ImageOperands => {
                let val = self.decoder.image_operands()?;
                let mut ops = vec![mr::Operand::ImageOperands(val)];
                ops.append(&mut self.parse_image_operands_arguments(val)?);
                ops
            }
            GOpKind::LoopControl => {
                let val = self.decoder.loop_control()?;
                let mut ops = vec![mr::Operand::LoopControl(val)];
                ops.append(&mut self.parse_loop_control_arguments(val)?);
                ops
            }
            GOpKind::MemoryAccess => {
                let val = self.decoder.memory_access()?;
                let mut ops = vec![mr::Operand::MemoryAccess(val)];
                ops.append(&mut self.parse_memory_access_arguments(val)?);
                ops
            }
            GOpKind::ExecutionMode => {
                let val = self.decoder.execution_mode()?;
                let mut ops = vec![mr::Operand::ExecutionMode(val)];
                ops.append(&mut self.parse_execution_mode_arguments(val)?);
                ops
            }
            GOpKind::Decoration => {
                let val = self.decoder.decoration()?;
                let mut ops = vec![mr::Operand::Decoration(val)];
                ops.append(&mut self.parse_decoration_arguments(val)?);
                ops
            }
            GOpKind::IdResultType => panic!(),  // not handled here
            GOpKind::IdResult => panic!(),  // not handled here
            GOpKind::LiteralContextDependentNumber => panic!(),  // not handled here
            GOpKind::LiteralSpecConstantOpInteger => panic!(),  // not handled here
        })
    }

    fn parse_image_operands_arguments(&mut self, image_operands: spirv::ImageOperands) -> Result<Vec<mr::Operand>> {
        let mut params = vec![];
        if image_operands.contains(spirv::ImageOperands::BIAS) {
            params.append(&mut vec![mr::Operand::IdRef(self.decoder.id()?)]);
        }
        if image_operands.contains(spirv::ImageOperands::LOD) {
            params.append(&mut vec![mr::Operand::IdRef(self.decoder.id()?)]);
        }
        if image_operands.contains(spirv::ImageOperands::GRAD) {
            params.append(&mut vec![mr::Operand::IdRef(self.decoder.id()?), mr::Operand::IdRef(self.decoder.id()?)]);
        }
        if image_operands.contains(spirv::ImageOperands::CONST_OFFSET) {
            params.append(&mut vec![mr::Operand::IdRef(self.decoder.id()?)]);
        }
        if image_operands.contains(spirv::ImageOperands::OFFSET) {
            params.append(&mut vec![mr::Operand::IdRef(self.decoder.id()?)]);
        }
        if image_operands.contains(spirv::ImageOperands::CONST_OFFSETS) {
            params.append(&mut vec![mr::Operand::IdRef(self.decoder.id()?)]);
        }
        if image_operands.contains(spirv::ImageOperands::SAMPLE) {
            params.append(&mut vec![mr::Operand::IdRef(self.decoder.id()?)]);
        }
        if image_operands.contains(spirv::ImageOperands::MIN_LOD) {
            params.append(&mut vec![mr::Operand::IdRef(self.decoder.id()?)]);
        }
        if image_operands.contains(spirv::ImageOperands::MAKE_TEXEL_AVAILABLE_KHR) {
            params.append(&mut vec![mr::Operand::IdScope(self.decoder.id()?)]);
        }
        if image_operands.contains(spirv::ImageOperands::MAKE_TEXEL_VISIBLE_KHR) {
            params.append(&mut vec![mr::Operand::IdScope(self.decoder.id()?)]);
        }
        Ok(params)
    }

    fn parse_loop_control_arguments(&mut self, loop_control: spirv::LoopControl) -> Result<Vec<mr::Operand>> {
        let mut params = vec![];
        if loop_control.contains(spirv::LoopControl::DEPENDENCY_LENGTH) {
            params.append(&mut vec![mr::Operand::LiteralInt32(self.decoder.int32()?)]);
        }
        Ok(params)
    }

    fn parse_memory_access_arguments(&mut self, memory_access: spirv::MemoryAccess) -> Result<Vec<mr::Operand>> {
        let mut params = vec![];
        if memory_access.contains(spirv::MemoryAccess::ALIGNED) {
            params.append(&mut vec![mr::Operand::LiteralInt32(self.decoder.int32()?)]);
        }
        if memory_access.contains(spirv::MemoryAccess::MAKE_POINTER_AVAILABLE_KHR) {
            params.append(&mut vec![mr::Operand::IdScope(self.decoder.id()?)]);
        }
        if memory_access.contains(spirv::MemoryAccess::MAKE_POINTER_VISIBLE_KHR) {
            params.append(&mut vec![mr::Operand::IdScope(self.decoder.id()?)]);
        }
        Ok(params)
    }

    fn parse_execution_mode_arguments(&mut self, execution_mode: spirv::ExecutionMode) -> Result<Vec<mr::Operand>> {
        Ok(match execution_mode {
            spirv::ExecutionMode::Invocations => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::ExecutionMode::LocalSize => vec![mr::Operand::LiteralInt32(self.decoder.int32()?), mr::Operand::LiteralInt32(self.decoder.int32()?), mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::ExecutionMode::LocalSizeHint => vec![mr::Operand::LiteralInt32(self.decoder.int32()?), mr::Operand::LiteralInt32(self.decoder.int32()?), mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::ExecutionMode::OutputVertices => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::ExecutionMode::VecTypeHint => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::ExecutionMode::SubgroupSize => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::ExecutionMode::SubgroupsPerWorkgroup => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::ExecutionMode::SubgroupsPerWorkgroupId => vec![mr::Operand::IdRef(self.decoder.id()?)],
            spirv::ExecutionMode::LocalSizeId => vec![mr::Operand::IdRef(self.decoder.id()?), mr::Operand::IdRef(self.decoder.id()?), mr::Operand::IdRef(self.decoder.id()?)],
            spirv::ExecutionMode::LocalSizeHintId => vec![mr::Operand::IdRef(self.decoder.id()?)],
            _ => vec![]
        })
    }

    fn parse_decoration_arguments(&mut self, decoration: spirv::Decoration) -> Result<Vec<mr::Operand>> {
        Ok(match decoration {
            spirv::Decoration::SpecId => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::ArrayStride => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::MatrixStride => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::BuiltIn => vec![mr::Operand::BuiltIn(self.decoder.built_in()?)],
            spirv::Decoration::Stream => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::Location => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::Component => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::Index => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::Binding => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::DescriptorSet => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::Offset => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::XfbBuffer => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::XfbStride => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::FuncParamAttr => vec![mr::Operand::FunctionParameterAttribute(self.decoder.function_parameter_attribute()?)],
            spirv::Decoration::FPRoundingMode => vec![mr::Operand::FPRoundingMode(self.decoder.fprounding_mode()?)],
            spirv::Decoration::FPFastMathMode => vec![mr::Operand::FPFastMathMode(self.decoder.fpfast_math_mode()?)],
            spirv::Decoration::LinkageAttributes => vec![mr::Operand::LiteralString(self.decoder.string()?), mr::Operand::LinkageType(self.decoder.linkage_type()?)],
            spirv::Decoration::InputAttachmentIndex => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::Alignment => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::MaxByteOffset => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::AlignmentId => vec![mr::Operand::IdRef(self.decoder.id()?)],
            spirv::Decoration::MaxByteOffsetId => vec![mr::Operand::IdRef(self.decoder.id()?)],
            spirv::Decoration::SecondaryViewportRelativeNV => vec![mr::Operand::LiteralInt32(self.decoder.int32()?)],
            spirv::Decoration::HlslCounterBufferGOOGLE => vec![mr::Operand::IdRef(self.decoder.id()?)],
            spirv::Decoration::HlslSemanticGOOGLE => vec![mr::Operand::LiteralString(self.decoder.string()?)],
            _ => vec![]
        })
    }
}