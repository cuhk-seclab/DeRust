mod send_sync_variance;
pub mod type_analysis;
mod unsafe_dataflow;
mod unsafe_destructor;

use snafu::{Error, ErrorCompat};

use crate::report::ReportLevel;

pub use send_sync_variance::{BehaviorFlag as SendSyncBehaviorFlag, SendSyncVarianceChecker};
pub use unsafe_dataflow::{
    BehaviorFlag as UnsafeDataflowBehaviorFlag, FunctionInputState, FunctionSources,
    TypeBehaviorFlag, UnsafeDataflowChecker,
};
pub use unsafe_destructor::UnsafeDestructorChecker;

pub type AnalysisResult<'tcx, T> = Result<T, Box<dyn AnalysisError + 'tcx>>;

use std::borrow::Cow;

pub trait AnalysisError: Error + ErrorCompat {
    fn kind(&self) -> AnalysisErrorKind;
    fn log(&self) {
        match self.kind() {
            AnalysisErrorKind::Unreachable => {
                error!("[{:?}] {}", self.kind(), self);
                if cfg!(feature = "backtraces") {
                    if let Some(backtrace) = ErrorCompat::backtrace(self) {
                        error!("Backtrace:\n{:?}", backtrace);
                    }
                }
            }
            AnalysisErrorKind::Unimplemented => {
                info!("[{:?}] {}", self.kind(), self);
                if cfg!(feature = "backtraces") {
                    if let Some(backtrace) = ErrorCompat::backtrace(self) {
                        info!("Backtrace:\n{:?}", backtrace);
                    }
                }
            }
            AnalysisErrorKind::OutOfScope => {
                debug!("[{:?}] {}", self.kind(), self);
                if cfg!(feature = "backtraces") {
                    if let Some(backtrace) = ErrorCompat::backtrace(self) {
                        debug!("Backtrace:\n{:?}", backtrace);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum AnalysisErrorKind {
    /// An error that should never happen;
    /// If this happens, that means some of our assumption / invariant is broken.
    /// Normal programs would panic for it, but we want to avoid panic at all cost,
    /// so this error exists.
    Unreachable,
    /// A pattern that is not handled by our algorithm yet.
    Unimplemented,
    /// An expected failure, something like "we don't handle this by design",
    /// that worth recording.
    OutOfScope,
}

// #[derive(Debug, Copy, Clone)]
#[derive(Debug, Clone)]
pub enum AnalysisKind {
    UnsafeDestructor,
    SendSyncVariance(SendSyncBehaviorFlag),
    UnsafeDataflow(UnsafeDataflowBehaviorFlag),
    UnsafeTypeDataflow(Vec<TypeBehaviorFlag>, Vec<TypeBehaviorFlag>),
}

trait IntoReportLevel {
    fn report_level(&self) -> ReportLevel;
}

impl Into<Cow<'static, str>> for AnalysisKind {
    fn into(self) -> Cow<'static, str> {
        match &self {
            AnalysisKind::UnsafeDestructor => "UnsafeDestructor".into(),
            AnalysisKind::SendSyncVariance(sv_analyses) => {
                let mut v = vec!["SendSyncVariance:"];
                if sv_analyses.contains(SendSyncBehaviorFlag::API_SEND_FOR_SYNC) {
                    v.push("ApiSendForSync")
                }
                if sv_analyses.contains(SendSyncBehaviorFlag::API_SYNC_FOR_SYNC) {
                    v.push("ApiSyncforSync")
                }
                if sv_analyses.contains(SendSyncBehaviorFlag::PHANTOM_SEND_FOR_SEND) {
                    v.push("PhantomSendForSend")
                }
                if sv_analyses.contains(SendSyncBehaviorFlag::NAIVE_SEND_FOR_SEND) {
                    v.push("NaiveSendForSend")
                }
                if sv_analyses.contains(SendSyncBehaviorFlag::NAIVE_SYNC_FOR_SYNC) {
                    v.push("NaiveSyncForSync")
                }
                if sv_analyses.contains(SendSyncBehaviorFlag::RELAX_SEND) {
                    v.push("RelaxSend")
                }
                if sv_analyses.contains(SendSyncBehaviorFlag::RELAX_SYNC) {
                    v.push("RelaxSync")
                }
                v.join("/").into()
            }
            AnalysisKind::UnsafeDataflow(bypass_kinds) => {
                let mut v = vec!["UnsafeDataflow:"];
                if bypass_kinds.contains(UnsafeDataflowBehaviorFlag::READ_FLOW) {
                    v.push("ReadFlow")
                }
                if bypass_kinds.contains(UnsafeDataflowBehaviorFlag::COPY_FLOW) {
                    v.push("CopyFlow")
                }
                if bypass_kinds.contains(UnsafeDataflowBehaviorFlag::VEC_FROM_RAW) {
                    v.push("VecFromRaw")
                }
                if bypass_kinds.contains(UnsafeDataflowBehaviorFlag::TRANSMUTE) {
                    v.push("Transmute")
                }
                if bypass_kinds.contains(UnsafeDataflowBehaviorFlag::WRITE_FLOW) {
                    v.push("WriteFlow")
                }
                if bypass_kinds.contains(UnsafeDataflowBehaviorFlag::PTR_AS_REF) {
                    v.push("PtrAsRef")
                }
                if bypass_kinds.contains(UnsafeDataflowBehaviorFlag::SLICE_UNCHECKED) {
                    v.push("SliceUnchecked")
                }
                if bypass_kinds.contains(UnsafeDataflowBehaviorFlag::SLICE_FROM_RAW) {
                    v.push("SliceFromRaw")
                }
                if bypass_kinds.contains(UnsafeDataflowBehaviorFlag::VEC_SET_LEN) {
                    v.push("VecSetLen")
                }
                if bypass_kinds.contains(UnsafeDataflowBehaviorFlag::TYPE_CONVERSION) {
                    v.push("TypeConversion")
                }
                v.join("/").into()
            }
            AnalysisKind::UnsafeTypeDataflow(sink_behaviors, source_behaviors) => {
                let mut v = vec!["UnsafeTypeDataflow:"];
                v.push("Sink behaviors:");
                if sink_behaviors.contains(&TypeBehaviorFlag::Dereference) {
                    v.push("Dereference")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::FunctionReturnValue) {
                    v.push("FunctionReturnValue")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::FunctionInputArgs) {
                    v.push("FunctionInputArg")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::ArrayIndexOutOfBound) {
                    v.push("ArrayIndexOutOfBound")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::ArrayCapacityOverflow) {
                    v.push("ArrayCapacityOverflow")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::ControlFlowDiverge) {
                    v.push("ControlFlowDiverge") // Negative index
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::PtrDropInPlace) {
                    v.push("PtrDropInPlace")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::PtrDirectDropInPlace) {
                    v.push("PtrDirectDropInPlace")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::IntrinsicsDropInPlace) {
                    v.push("IntrinsicsDropInPlace")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::PtrRead) {
                    v.push("PtrRead")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::PtrDirectRead) {
                    v.push("PtrDirectRead")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::IntrinsicsCopy) {
                    v.push("IntrinsicsCopy")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::IntrinsicsCopyNonoverlapping) {
                    v.push("IntrinsicsCopyNonoverlapping")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::PtrWrite) {
                    v.push("PtrWrite")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::PtrDirectWrite) {
                    v.push("PtrDirectWrite")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::SliceGetUnchecked) {
                    v.push("SliceGetUnchecked")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::SliceGetUncheckedMut) {
                    v.push("SliceGetUncheckedMut")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::VecFromElem) {
                    v.push("VecFromElem")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::VecIndex) {
                    v.push("VecIndex")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::StrGetUnchecked) {
                    v.push("StrGetUnchecked")
                }
                if sink_behaviors.contains(&TypeBehaviorFlag::StrGetUncheckedMut) {
                    v.push("StrGetUncheckedMut")
                }

                v.push("caused by Source behaviors:");

                if source_behaviors.contains(&TypeBehaviorFlag::ImmutPtrToMutPtr) {
                    v.push("ImmutPtrToMutPtr")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::ImmutRefToMutRef) {
                    v.push("ImmutRefToMutRef")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::AddressOf) {
                    v.push("AddressOf")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::Transmute) {
                    v.push("Transmute")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::ConcretizedToGeneric) {
                    v.push("ConcretizedToGeneric")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::GenericToConcretized) {
                    v.push("GenericToConcretized")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::GenericToGeneric) {
                    v.push("GenericToGeneric")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::Lifetime) {
                    v.push("Lifetime")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::BigToSmallIntToInt) {
                    v.push("BigToSmallIntToInt")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::BigToSmallUintToUint) {
                    v.push("BigToSmallUintToUint")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::BigToSmallIntToUint) {
                    v.push("BigToSmallIntToUint")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::BigToSmallUintToInt) {
                    v.push("BigToSmallUintToInt")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::SmallToBigIntToInt) {
                    v.push("SmallToBigIntToInt")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::SmallToBigUintToInt) {
                    v.push("SmallToBigUintToInt")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::SmallToBigUintToUint) {
                    v.push("SmallToBigUintToUint")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::SmallToBigIntToUint) {
                    v.push("SmallToBigIntToUint")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::SmallToBigSizePrimitive) {
                    v.push("SmallToBigSizePrimitive")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::BigToSmallSizePrimitive) {
                    v.push("BigToSmallSizePrimitive")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::DifferentPrimitiveType) {
                    v.push("DifferentPrimitiveType")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::SmallToBigSizeSequence) {
                    v.push("SmallToBigSizeSequence")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::BigToSmallSizeSequence) {
                    v.push("BigToSmallSizeSequence")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::DifferentSequenceType) {
                    v.push("DifferentSequenceType")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::SmallToBigSizeRawPtr) {
                    v.push("SmallToBigSizeRawPtr")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::BigToSmallSizeRawPtr) {
                    v.push("BigToSmallSizeRawPtr")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::DifferentRawPtrType) {
                    v.push("DifferentRawPtrType")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::SmallToBigSizeRef) {
                    v.push("SmallToBigSizeRef")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::BigToSmallSizeRef) {
                    v.push("BigToSmallSizeRef")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::DifferentRefType) {
                    v.push("DifferentRefType")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::DifferentAdtType) {
                    v.push("DifferentAdtType")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::DifferentArrayType) {
                    v.push("DifferentArrayType")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::DifferentTupleType) {
                    v.push("DifferentTupleType")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::VecFromRawParts) {
                    v.push("VecFromRawParts")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::PtrAsRef) {
                    v.push("PtrAsRef")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::PtrAsMut) {
                    v.push("PtrAsMut")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::NonNullAsRef) {
                    v.push("NonNullAsRef")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::NonNullAsMut) {
                    v.push("NonNullAsMut")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::PtrSliceFromRawParts) {
                    v.push("PtrSliceFromRawParts")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::PtrSliceFromRawPartsMut) {
                    v.push("PtrSliceFromRawPartsMut")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::SliceFromRawParts) {
                    v.push("SliceFromRawParts")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::SliceFromRawPartsMut) {
                    v.push("SliceFromRawPartsMut")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::StringFromRawParts) {
                    v.push("StringFromRawParts")
                }
                if source_behaviors.contains(&TypeBehaviorFlag::BoxFromRaw) {
                    v.push("BoxFromRaw")
                }
                v.join(" ").into()
            }
        }
    }
}
