pub use self::attr::*;
pub use self::class::*;
pub use self::expr::*;
pub use self::func::*;
pub use self::imp::*;
pub use self::path::*;
pub use self::stmt::*;
pub use self::struc::*;
pub use self::ty::*;
pub use self::using::*;

use crate::lexer::{
    ClassKeyword, FnKeyword, Identifier, ImplKeyword, Lexer, StructKeyword, SyntaxError, Token,
    UseKeyword,
};
use std::path::PathBuf;
use thiserror::Error;

mod attr;
mod class;
mod expr;
mod func;
mod imp;
mod path;
mod stmt;
mod struc;
mod ty;
mod using;

///  A parsed source file.
pub struct SourceFile {
    path: PathBuf,
    uses: Vec<Use>,
    ty: Option<TypeDefinition>,
    impls: Vec<TypeImpl>,
}

impl SourceFile {
    pub fn parse<P: Into<PathBuf>>(path: P) -> Result<SourceFile, ParseError> {
        // Read the file.
        let path = path.into();
        let data = match std::fs::read_to_string(&path) {
            Ok(v) => v,
            Err(e) => return Err(ParseError::ReadFailed(e)),
        };

        // Parse source file.
        let mut file = Self {
            path,
            ty: None,
            uses: Vec::new(),
            impls: Vec::new(),
        };

        if let Err(e) = file.parse_top(data) {
            return Err(ParseError::ParseFailed(e));
        }

        Ok(file)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn ty(&self) -> Option<&TypeDefinition> {
        self.ty.as_ref()
    }

    pub fn impls(&self) -> &[TypeImpl] {
        self.impls.as_ref()
    }

    fn parse_top(&mut self, data: String) -> Result<(), SyntaxError> {
        let mut lex = Lexer::new(data);
        let mut attrs = None;

        loop {
            // Get next token.
            let tok = match lex.next()? {
                Some(v) => v,
                None => break,
            };

            // Check token.
            match tok {
                Token::AttributeName(name) => attrs = Some(Attributes::parse(&mut lex, name)?),
                Token::UseKeyword(def) => self.uses.push(Self::parse_use(&mut lex, def)?),
                Token::StructKeyword(def) => {
                    let name = lex.next_ident()?;
                    self.can_define_type(&name)?;
                    self.ty = Some(TypeDefinition::Struct(Self::parse_struct(
                        &mut lex,
                        attrs.take().unwrap_or_default(),
                        def,
                        name,
                    )?));
                }
                Token::ClassKeyword(def) => {
                    let name = lex.next_ident()?;
                    self.can_define_type(&name)?;
                    self.ty = Some(TypeDefinition::Class(Self::parse_class(
                        &mut lex,
                        attrs.take().unwrap_or_default(),
                        def,
                        name,
                    )?));
                }
                Token::ImplKeyword(def) => {
                    let ty = lex.next_ident()?;
                    let tok = match lex.next()? {
                        Some(v) => v,
                        None => {
                            return Err(SyntaxError::new(
                                ty.span().clone(),
                                "expect '{' after this",
                            ));
                        }
                    };

                    match tok {
                        Token::OpenCurly(_) => {
                            match &self.ty {
                                Some(v) => {
                                    if *v.name() != ty {
                                        return Err(SyntaxError::new(
                                            ty.span().clone(),
                                            "an implementation is not matched with type in the file"
                                        ));
                                    }
                                }
                                None => {
                                    return Err(SyntaxError::new(
                                        ty.span().clone(),
                                        "type must be defined before define an implementation",
                                    ));
                                }
                            }

                            self.impls.push(Self::parse_type_impl(&mut lex, def, ty)?);
                        }
                        t => return Err(SyntaxError::new(t.span().clone(), "expect '{'")),
                    }
                }
                t => {
                    return Err(SyntaxError::new(
                        t.span().clone(),
                        "this item is not allowed as a top-level",
                    ));
                }
            }
        }

        Ok(())
    }

    fn parse_use(lex: &mut Lexer, def: UseKeyword) -> Result<Use, SyntaxError> {
        // Get the package name.
        let mut name = Vec::new();

        match lex.next()? {
            Some(Token::SelfKeyword(v)) => name.push(Token::SelfKeyword(v)),
            Some(Token::Identifier(v)) => name.push(Token::Identifier(v)),
            Some(t) => {
                return Err(SyntaxError::new(
                    t.span().clone(),
                    "expect an identifer or self keyword",
                ));
            }
            None => {
                return Err(SyntaxError::new(
                    def.span().clone(),
                    "expect an identifer or self keyword after this",
                ));
            }
        }

        match lex.next()? {
            Some(Token::FullStop(v)) => name.push(Token::FullStop(v)),
            Some(t) => return Err(SyntaxError::new(t.span().clone(), "expect '.'")),
            None => {
                return Err(SyntaxError::new(
                    lex.last().unwrap().clone(),
                    "expect '.' after this",
                ));
            }
        }

        // Get item after the package name.
        match lex.next()? {
            Some(Token::Identifier(v)) => name.push(Token::Identifier(v)),
            Some(t) => return Err(SyntaxError::new(t.span().clone(), "expect an identifer")),
            None => {
                return Err(SyntaxError::new(
                    lex.last().unwrap().clone(),
                    "expect an identifer after this",
                ));
            }
        }

        // Get remaining path.
        loop {
            let next = match lex.next()? {
                Some(v) => v,
                None => {
                    return Err(SyntaxError::new(
                        lex.last().unwrap().clone(),
                        "expect ';' after this",
                    ));
                }
            };

            match next {
                Token::FullStop(v) => {
                    name.push(Token::FullStop(v));

                    match lex.next()? {
                        Some(Token::Identifier(v)) => name.push(Token::Identifier(v)),
                        Some(t) => {
                            return Err(SyntaxError::new(t.span().clone(), "expect an identifer"));
                        }
                        None => {
                            return Err(SyntaxError::new(
                                lex.last().unwrap().clone(),
                                "expect an identifer after this",
                            ));
                        }
                    }
                }
                Token::Semicolon(_) => break,
                t => return Err(SyntaxError::new(t.span().clone(), "expect ';'")),
            }
        }

        Ok(Use::new(def, Path::new(name), None))
    }

    fn parse_struct(
        lex: &mut Lexer,
        attrs: Attributes,
        def: StructKeyword,
        name: Identifier,
    ) -> Result<Struct, SyntaxError> {
        // Check if a primitive struct.
        if let Some((_, repr)) = attrs.repr() {
            let repr = *repr;
            lex.next_semicolon()?;
            return Ok(Struct::Primitive(attrs, repr, def, name));
        }

        // Parse fields.
        lex.next_oc()?;

        loop {
            let tok = match lex.next()? {
                Some(v) => v,
                None => {
                    return Err(SyntaxError::new(
                        lex.last().unwrap().clone(),
                        "expect '}' after this",
                    ));
                }
            };

            match tok {
                Token::CloseCurly(_) => break,
                t => return Err(SyntaxError::new(t.span().clone(), "expect '}'")),
            }
        }

        Ok(Struct::Composite(attrs, def, name))
    }

    fn parse_class(
        lex: &mut Lexer,
        attrs: Attributes,
        def: ClassKeyword,
        name: Identifier,
    ) -> Result<Class, SyntaxError> {
        // Check if zero-sized class. A zero-sized class cannot be instantiate. They act as a
        // container for static methods.
        let tok = match lex.next()? {
            Some(v) => v,
            None => {
                return Err(SyntaxError::new(
                    name.span().clone(),
                    "expect either ';' or '{' after the class name",
                ));
            }
        };

        match tok {
            Token::Semicolon(_) => return Ok(Class::new(attrs, def, name)),
            Token::OpenCurly(_) => {}
            v => {
                return Err(SyntaxError::new(
                    v.span().clone(),
                    "expect either ';' or '{'",
                ));
            }
        }

        // Parse fields.
        loop {
            let tok = match lex.next()? {
                Some(v) => v,
                None => {
                    return Err(SyntaxError::new(
                        lex.last().unwrap().clone(),
                        "expect an '}'",
                    ));
                }
            };

            match tok {
                Token::CloseCurly(_) => break,
                t => return Err(SyntaxError::new(t.span().clone(), "syntax error")),
            }
        }

        Ok(Class::new(attrs, def, name))
    }

    fn parse_type_impl(
        lex: &mut Lexer,
        def: ImplKeyword,
        ty: Identifier,
    ) -> Result<TypeImpl, SyntaxError> {
        let mut attrs = None;
        let mut functions = Vec::new();

        loop {
            let tok = match lex.next()? {
                Some(v) => v,
                None => {
                    return Err(SyntaxError::new(
                        lex.last().unwrap().clone(),
                        "expect an '}'",
                    ));
                }
            };

            match tok {
                Token::AttributeName(name) => attrs = Some(Attributes::parse(lex, name)?),
                Token::FnKeyword(def) => {
                    functions.push(Self::parse_fn(lex, attrs.take().unwrap_or_default(), def)?);
                }
                Token::CloseCurly(_) => break,
                t => return Err(SyntaxError::new(t.span().clone(), "syntax error")),
            }
        }

        Ok(TypeImpl::new(def, ty, functions))
    }

    fn parse_fn(
        lex: &mut Lexer,
        attrs: Attributes,
        def: FnKeyword,
    ) -> Result<Function, SyntaxError> {
        let name = lex.next_ident()?;

        // Parse parameters.
        let mut params = Vec::new();

        lex.next_op()?;

        loop {
            let tok = match lex.next()? {
                Some(v) => v,
                None => {
                    return Err(SyntaxError::new(
                        lex.last().unwrap().clone(),
                        "expect an ')'",
                    ));
                }
            };

            match tok {
                Token::Identifier(name) => {
                    // Parse the parameter.
                    lex.next_colon()?;
                    params.push(FunctionParam::new(name, Self::parse_type(lex)?));

                    // Check for a ','.
                    let tok = match lex.next()? {
                        Some(v) => v,
                        None => {
                            return Err(SyntaxError::new(
                                lex.last().unwrap().clone(),
                                "expect an ')'",
                            ));
                        }
                    };

                    match tok {
                        Token::Comma(_) => {}
                        Token::CloseParenthesis(_) => break,
                        t => return Err(SyntaxError::new(t.span().clone(), "syntax error")),
                    }
                }
                Token::CloseParenthesis(_) => break,
                t => return Err(SyntaxError::new(t.span().clone(), "syntax error")),
            }
        }

        // Parse return type.
        let next = match lex.next()? {
            Some(v) => v,
            None => {
                return Err(SyntaxError::new(
                    lex.last().unwrap().clone(),
                    "expect either '{' or ';' after this",
                ));
            }
        };

        let ret = match next {
            Token::Semicolon(_) => return Ok(Function::new(attrs, def, name, params, None, None)),
            Token::OpenCurly(_) => None,
            Token::Colon(_) => {
                let ret = Self::parse_type(lex)?;
                let next = match lex.next()? {
                    Some(v) => v,
                    None => {
                        return Err(SyntaxError::new(
                            lex.last().unwrap().clone(),
                            "expect either '{' or ';' after this",
                        ));
                    }
                };

                match next {
                    Token::Semicolon(_) => {
                        return Ok(Function::new(attrs, def, name, params, Some(ret), None));
                    }
                    Token::OpenCurly(_) => {}
                    t => {
                        return Err(SyntaxError::new(
                            t.span().clone(),
                            "expect either '{' or ';'",
                        ));
                    }
                }

                Some(ret)
            }
            t => {
                return Err(SyntaxError::new(
                    t.span().clone(),
                    "expect either '{' or ';'",
                ));
            }
        };

        // Parse body.
        let body = Statement::parse_block(lex)?;

        Ok(Function::new(attrs, def, name, params, ret, Some(body)))
    }

    fn parse_type(lex: &mut Lexer) -> Result<Type, SyntaxError> {
        // Parse pointer prefix.
        let mut prefixes = Vec::new();

        loop {
            match lex.next()? {
                Some(Token::Asterisk(v)) => prefixes.push(v),
                Some(_) => {
                    lex.undo();
                    break;
                }
                None => {
                    return Err(SyntaxError::new(
                        lex.last().unwrap().clone(),
                        "expect an identifier after this",
                    ));
                }
            }
        }

        // Parse type.
        let next = match lex.next()? {
            Some(v) => v,
            None => {
                return Err(SyntaxError::new(
                    lex.last().unwrap().clone(),
                    "expect an identifier after this",
                ));
            }
        };

        let name = match next {
            Token::ExclamationMark(v) => TypeName::Never(v),
            Token::OpenParenthesis(o) => TypeName::Unit(o, lex.next_cp()?),
            Token::Identifier(mut ident) => {
                let mut fqtn = Vec::new();

                loop {
                    match lex.next()? {
                        Some(Token::FullStop(v)) => {
                            fqtn.push(Token::FullStop(v));
                            fqtn.push(Token::Identifier(ident));
                        }
                        Some(_) => {
                            lex.undo();
                            break;
                        }
                        None => break,
                    }

                    ident = match lex.next()? {
                        Some(Token::Identifier(v)) => v,
                        Some(t) => {
                            return Err(SyntaxError::new(t.span().clone(), "expect an identifier"))
                        }
                        None => {
                            return Err(SyntaxError::new(
                                lex.last().unwrap().clone(),
                                "expect an identifier after this",
                            ));
                        }
                    };
                }

                fqtn.push(Token::Identifier(ident));

                TypeName::Ident(Path::new(fqtn))
            }
            t => return Err(SyntaxError::new(t.span().clone(), "invalid type")),
        };

        Ok(Type::new(prefixes, name))
    }

    fn can_define_type(&self, name: &Identifier) -> Result<(), SyntaxError> {
        if self.ty.is_some() {
            return Err(SyntaxError::new(
                name.span().clone(),
                "multiple type definition in a source file",
            ));
        } else if name.value() != self.path.file_stem().unwrap() {
            return Err(SyntaxError::new(
                name.span().clone(),
                "type name and file name must be matched",
            ));
        }

        Ok(())
    }
}

/// A type definition in a source file.
pub enum TypeDefinition {
    Struct(Struct),
    Class(Class),
}

impl TypeDefinition {
    pub fn name(&self) -> &Identifier {
        match self {
            Self::Struct(v) => v.name(),
            Self::Class(v) => v.name(),
        }
    }
}

/// Represents an error when [`SourceFile::parse()`] is failed.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("cannot read source file")]
    ReadFailed(#[source] std::io::Error),

    #[error("cannot parse source file")]
    ParseFailed(#[source] SyntaxError),
}
