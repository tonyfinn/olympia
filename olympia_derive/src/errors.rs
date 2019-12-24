use proc_macro2::Span;
use syn::spanned::Spanned;
use thiserror::Error;

pub(crate) fn merge_syn_errors(errors: &[syn::Error]) -> Option<syn::Error> {
    let first = errors.get(0)?.clone();
    Some(errors.iter().skip(1).fold(first, |mut acc, err| {
        acc.combine(err.clone());
        acc
    }))
}

/// Trait for errors that can be grouped together if multiple exist.
///
/// The use case is to report all errors with a given input to the user.
pub(crate) trait DeriveError: std::error::Error + Sized + Clone {
    /// Take a vector of this error type and return a variant
    /// that represents a grouped error.
    fn compress(errs: Vec<Self>) -> Self;

    /// If this error type contains multiple suberrors, return those errors
    fn suberrors(&self) -> Option<Vec<Self>>;

    /// If this error represents a specific subspan of the object being considered
    fn subspan(&self) -> Option<Span>;

    /// Access underyling syn::Error if this was generated from syn
    fn wrapped_syn_error(&self) -> Option<&syn::Error>;

    /// Group a list of errors into a single result variant.
    ///
    /// If there are no errors, returns Ok(())
    ///
    /// If there is a single item, returns that items
    ///
    /// If there are multiple items, returns the multiple item variant. If any of
    /// those items themselves are grouped errors, this will flatten out one layer.
    fn ok_or_group_errors(errs: Vec<Self>) -> Result<(), Self> {
        if errs.is_empty() {
            Ok(())
        } else if errs.len() == 1 {
            Err(errs[0].clone())
        } else {
            let mut grouped_errors = Vec::new();
            for err in errs {
                if let Some(suberrors) = err.suberrors() {
                    grouped_errors.extend(suberrors)
                } else {
                    grouped_errors.push(err.clone());
                }
            }
            Err(Self::compress(grouped_errors))
        }
    }

    fn to_syn_error(&self, default_span: proc_macro2::Span) -> syn::Error {
        match self.suberrors() {
            Some(errors) => {
                let mapped_errors: Vec<syn::Error> = errors
                    .into_iter()
                    .map(|err| err.to_syn_error(default_span))
                    .collect();
                merge_syn_errors(&mapped_errors).unwrap()
            }
            None => {
                if let Some(wrapped) = self.wrapped_syn_error() {
                    wrapped.clone()
                } else {
                    syn::Error::new(
                        match self.subspan() {
                            Some(sp) => sp,
                            None => default_span,
                        },
                        self,
                    )
                }
            }
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct GroupedError<T> {
    errors: Vec<T>,
}

impl<T> GroupedError<T> {
    fn new(errors: Vec<T>) -> GroupedError<T> {
        GroupedError { errors }
    }
}

impl<T> std::fmt::Display for GroupedError<T>
where
    T: DeriveError + std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in self.errors.iter() {
            write!(f, "{}", item)?
        }
        Ok(())
    }
}

#[derive(Clone, Error, Debug)]
pub(crate) enum ParamError {
    #[error("Unrecognised field deriving instruction param '{0:?}'")]
    UnknownField(syn::Path),
    #[error("{0:?} is not a supported type for embedded params")]
    UnsupportedEmbeddedType(Box<syn::Type>),
    #[error("{0:?} is not a supported type for appended params")]
    UnsupportedAppendedType(Box<syn::Type>),
    #[error("{0:?} is not a supported type for constant params")]
    UnsupportedConstantType(Box<syn::Type>),
    #[error("Must provide one and only one constant value, found: {0:?}")]
    MultipleConstantValues(syn::MetaList),
    #[error("Must provide a identifier for constant values, found: {0:?}")]
    ConstantNotPath(syn::MetaList),
    #[error("Invalid parameter mask")]
    InvalidParamMask(proc_macro2::Span),
    #[error("Constant Parameters should not have a mask")]
    MaskAndConstant(proc_macro2::Span),
    #[error("Position not specified (use src, dest or single)")]
    MissingPosition(proc_macro2::Span),
    #[error("Unexpected literal {0:?}")]
    UnexpectedLiteral(syn::Lit),
    #[error("Error parsing param: '{0}'")]
    SynError(#[from] syn::Error),
    #[error("Errors encountered: {0}")]
    Multiple(GroupedError<ParamError>),
}

impl DeriveError for ParamError {
    fn subspan(&self) -> Option<Span> {
        match self {
            ParamError::UnknownField(path) => Some(path.span()),
            ParamError::UnsupportedEmbeddedType(ty) => Some(ty.span()),
            ParamError::UnsupportedAppendedType(ty) => Some(ty.span()),
            ParamError::MultipleConstantValues(ml) => Some(ml.span()),
            ParamError::InvalidParamMask(sp) => Some(*sp),
            ParamError::MaskAndConstant(sp) => Some(*sp),
            ParamError::MissingPosition(sp) => Some(*sp),
            ParamError::UnexpectedLiteral(lit) => Some(lit.span()),
            ParamError::SynError(err) => Some(err.span()),
            _ => None,
        }
    }

    fn suberrors(&self) -> Option<Vec<ParamError>> {
        match self {
            ParamError::Multiple(group) => Some(group.errors.clone()),
            _ => None,
        }
    }

    fn compress(errs: Vec<ParamError>) -> ParamError {
        ParamError::Multiple(GroupedError::new(errs))
    }

    fn wrapped_syn_error(&self) -> Option<&syn::Error> {
        match self {
            ParamError::SynError(wrapped) => Some(wrapped),
            _ => None,
        }
    }
}

#[derive(Clone, Error, Debug)]
pub(crate) enum InstructionError {
    #[error("Unrecognised olympia field at instruction level '{0:?}'")]
    UnknownField(syn::Path),
    #[error("Must provide an opcode mask at instruction level")]
    MissingOpcodeMask,
    #[error("Opcodes must be numeric and 8 digits of hex")]
    InvalidOpcodeMask(syn::Lit),
    #[error("Must provide a label at instruction level")]
    MissingLabel,
    #[error("Must provide at least a label and opcode for an instruction")]
    MissingPrereq,
    #[error("Unexpected literal {0:?}")]
    UnexpectedLiteral(syn::Lit),
    #[error("Error parsing instruction: '{0}'")]
    SynError(#[from] syn::Error),
    #[error("Errors encountered: {0}")]
    Multiple(GroupedError<InstructionError>),
    #[error("Can only derive instructions on a struct")]
    NotAStruct,
}

impl DeriveError for InstructionError {
    fn subspan(&self) -> Option<Span> {
        match self {
            InstructionError::UnknownField(path) => Some(path.span()),
            InstructionError::InvalidOpcodeMask(lit) => Some(lit.span()),
            InstructionError::UnexpectedLiteral(lit) => Some(lit.span()),
            InstructionError::SynError(err) => Some(err.span()),
            _ => None,
        }
    }

    fn suberrors(&self) -> Option<Vec<InstructionError>> {
        match self {
            InstructionError::Multiple(group) => Some(group.errors.clone()),
            _ => None,
        }
    }

    fn compress(errs: Vec<InstructionError>) -> InstructionError {
        InstructionError::Multiple(GroupedError::new(errs))
    }

    fn wrapped_syn_error(&self) -> Option<&syn::Error> {
        match self {
            InstructionError::SynError(wrapped) => Some(wrapped),
            _ => None,
        }
    }
}

#[derive(Clone, Error, Debug)]
pub(crate) enum DeriveErrorEnum {
    #[error("{0}")]
    Instruction(#[from] InstructionError),
    #[error("{0}")]
    Param(#[from] ParamError),
}

impl DeriveError for DeriveErrorEnum {
    fn subspan(&self) -> Option<Span> {
        match self {
            DeriveErrorEnum::Instruction(err) => err.subspan(),
            DeriveErrorEnum::Param(err) => err.subspan(),
        }
    }

    fn suberrors(&self) -> Option<Vec<DeriveErrorEnum>> {
        None
    }

    fn compress(_errs: Vec<DeriveErrorEnum>) -> DeriveErrorEnum {
        unreachable!()
    }

    fn wrapped_syn_error(&self) -> Option<&syn::Error> {
        match self {
            DeriveErrorEnum::Instruction(err) => err.wrapped_syn_error(),
            DeriveErrorEnum::Param(err) => err.wrapped_syn_error(),
        }
    }
}

pub(crate) type InstructionResult<T> = Result<T, InstructionError>;
pub(crate) type ParamResult<T> = Result<T, ParamError>;
pub(crate) type DeriveResult<T> = Result<T, DeriveErrorEnum>;
