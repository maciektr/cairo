use cairo_lang_defs::diagnostic_utils::StableLocation;
use cairo_lang_diagnostics::{
    DiagnosticAdded, DiagnosticEntry, DiagnosticLocation, DiagnosticNote, Diagnostics,
    DiagnosticsBuilder,
};
use cairo_lang_filesystem::ids::FileId;
use cairo_lang_semantic::corelib::LiteralError;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::expr::inference::InferenceError;
use cairo_lang_syntax::node::ids::SyntaxStablePtrId;

use crate::Location;

pub struct LoweringDiagnostics {
    pub diagnostics: DiagnosticsBuilder<LoweringDiagnostic>,
    pub file_id: FileId,
}
impl LoweringDiagnostics {
    pub fn new(file_id: FileId) -> Self {
        Self { file_id, diagnostics: DiagnosticsBuilder::default() }
    }
    pub fn build(self) -> Diagnostics<LoweringDiagnostic> {
        self.diagnostics.build()
    }
    pub fn report(
        &mut self,
        stable_ptr: SyntaxStablePtrId,
        kind: LoweringDiagnosticKind,
    ) -> DiagnosticAdded {
        self.report_by_location(Location::new(StableLocation::new(stable_ptr)), kind)
    }
    pub fn report_by_location(
        &mut self,
        location: Location,
        kind: LoweringDiagnosticKind,
    ) -> DiagnosticAdded {
        self.diagnostics.add(LoweringDiagnostic { location, kind })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LoweringDiagnostic {
    pub location: Location,
    pub kind: LoweringDiagnosticKind,
}
impl DiagnosticEntry for LoweringDiagnostic {
    type DbType = dyn SemanticGroup;

    fn format(&self, db: &Self::DbType) -> String {
        match &self.kind {
            LoweringDiagnosticKind::Unreachable { .. } => "Unreachable code".into(),
            LoweringDiagnosticKind::VariableMoved { .. } => "Variable was previously moved.".into(),
            LoweringDiagnosticKind::VariableNotDropped { .. } => "Variable not dropped.".into(),
            LoweringDiagnosticKind::DesnappingANonCopyableType { .. } => {
                "Cannot desnap a non copyable type.".into()
            }
            LoweringDiagnosticKind::UnsupportedMatchedType(matched_type) =>
                format!("Unsupported matched type. Type: `{}`.", matched_type),
            LoweringDiagnosticKind::UnsupportedMatchedValueTuple => "Unsupported matched value. \
                Currently, match on tuples only supports enums as tuple members."
                .into(),
            LoweringDiagnosticKind::UnsupportedMatchArmNotAVariant => {
                "Unsupported match arm - not a variant.".into()
            }
            LoweringDiagnosticKind::UnsupportedMatchArmNotALiteral => {
                "Unsupported match arm - not a literal.".into()
            }
            LoweringDiagnosticKind::UnsupportedMatchArmNonSequential => {
                "Unsupported match - numbers must be sequential starting from 0.".into()
            }
            LoweringDiagnosticKind::UnsupportedMatchArmOrNotSupported => {
                "Unsupported match arm - or pattern is not supported in this context".into()
            }
            LoweringDiagnosticKind::UnsupportedMatchArmNotATuple => {
                "Unsupported match arm - not a tuple.".into()
            }
            LoweringDiagnosticKind::NonExhaustiveMatchFelt252 => {
                "Match is non exhaustive - match over a numerical value must have a wildcard card pattern (`_`)."
                    .into()
            }
            LoweringDiagnosticKind::CannotInlineFunctionThatMightCallItself => {
                "Cannot inline a function that might call itself.".into()
            }
            LoweringDiagnosticKind::MemberPathLoop => {
                "Currently, loops must change the entire variable.".into()
            }
            LoweringDiagnosticKind::UnexpectedError => {
                "Unexpected error has occurred, Please submit a full bug report. \
                See https://github.com/starkware-libs/cairo/issues/new/choose for instructions.\
                "
                .into()
            }
            LoweringDiagnosticKind::NoPanicFunctionCycle => {
                "Call cycle of `nopanic` functions is not allowed.".into()
            },
            LoweringDiagnosticKind::LiteralError(literal_error) => literal_error.format(db),
            LoweringDiagnosticKind::UnsupportedPattern => {
                "Inner patterns are not in this context.".into()
            }
            LoweringDiagnosticKind::MissingMatchArm(variant) => format!("Missing match arm: `{}` not covered.", variant),
            LoweringDiagnosticKind::UnreachableMatchArm => "Unreachable pattern arm.".into(),
            LoweringDiagnosticKind::Unsupported => "Unsupported feature.".into(),
        }
    }

    fn notes(&self, _db: &Self::DbType) -> &[DiagnosticNote] {
        &self.location.notes
    }

    #[allow(unreachable_patterns, clippy::single_match)]
    fn location(&self, db: &Self::DbType) -> DiagnosticLocation {
        match &self.kind {
            LoweringDiagnosticKind::Unreachable { last_statement_ptr } => {
                return self
                    .location
                    .stable_location
                    .diagnostic_location_until(db.upcast(), *last_statement_ptr);
            }
            _ => {}
        }
        self.location.stable_location.diagnostic_location(db.upcast())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LoweringDiagnosticKind {
    Unreachable { last_statement_ptr: SyntaxStablePtrId },
    VariableMoved { inference_error: InferenceError },
    VariableNotDropped { drop_err: InferenceError, destruct_err: InferenceError },
    DesnappingANonCopyableType { inference_error: InferenceError },
    UnsupportedMatchedType(String),
    UnsupportedMatchedValueTuple,
    MissingMatchArm(String),
    UnreachableMatchArm,
    UnexpectedError,
    UnsupportedMatchArmNotAVariant,
    UnsupportedMatchArmNotALiteral,
    UnsupportedMatchArmNotATuple,
    UnsupportedMatchArmNonSequential,
    UnsupportedMatchArmOrNotSupported,
    NonExhaustiveMatchFelt252,
    CannotInlineFunctionThatMightCallItself,
    MemberPathLoop,
    NoPanicFunctionCycle,
    LiteralError(LiteralError),
    UnsupportedPattern,
    Unsupported,
}
