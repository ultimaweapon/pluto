use super::{Path, Use};
use crate::codegen::{Codegen, LlvmType, LlvmVoid};
use crate::lexer::{
    Asterisk, CloseParenthesis, ExclamationMark, OpenParenthesis, Span, SyntaxError,
};

/// A type of something (e.g. variable).
pub struct Type {
    prefixes: Vec<Asterisk>,
    name: TypeName,
}

impl Type {
    pub fn new(prefixes: Vec<Asterisk>, name: TypeName) -> Self {
        Self { prefixes, name }
    }

    pub fn name(&self) -> &TypeName {
        &self.name
    }

    pub fn build<'a, 'b: 'a>(
        &self,
        cx: &'a Codegen<'b>,
        uses: &[Use],
    ) -> Result<Option<LlvmType<'a, 'b>>, SyntaxError> {
        let mut ty = match &self.name {
            TypeName::Unit(_, _) => Some(LlvmType::Void(LlvmVoid::new(cx))),
            TypeName::Never(_) => None,
            TypeName::Ident(n) => match cx.resolve(uses, n) {
                Some(v) => Some(v),
                None => return Err(SyntaxError::new(n.span(), "undefined type")),
            },
        };

        // TODO: Resolve pointers.
        Ok(ty)
    }
}

/// Name of a [`Type`].
pub enum TypeName {
    Unit(OpenParenthesis, CloseParenthesis),
    Never(ExclamationMark),
    Ident(Path),
}

impl TypeName {
    pub fn span(&self) -> Span {
        match self {
            TypeName::Unit(o, c) => o.span() + c.span(),
            TypeName::Never(v) => v.span().clone(),
            TypeName::Ident(v) => v.span(),
        }
    }
}
