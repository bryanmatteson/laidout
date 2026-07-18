//! Go-like abstract syntax tree corpus ported from the prototype.
//!
//! Function signatures contain nested, asymmetric choices: parameters and
//! results can break independently while the declaration and body retain
//! their own alternatives. That makes this a stronger optimality workload
//! than uniformly nested JSON groups.

use std::rc::Rc;

use crate::doc::{concat, group, hardline, join, line, nest, softline, tag, text, Doc, TagId};
use crate::{tags, tokens};

pub const TAG_KEYWORD: TagId = tags::CUSTOM_START + 8;
pub const TAG_IDENT: TagId = tags::CUSTOM_START + 9;
pub const TAG_TYPE: TagId = tags::CUSTOM_START + 10;
pub const TAG_STRING: TagId = tags::CUSTOM_START + 11;
pub const TAG_STATEMENT: TagId = tags::CUSTOM_START + 12;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct File {
    pub package: String,
    pub imports: Vec<Import>,
    pub declarations: Vec<FuncDecl>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Import {
    pub name: Option<String>,
    pub path: String,
}

impl Import {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            name: None,
            path: path.into(),
        }
    }

    pub fn named(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            path: path.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: String,
}

impl Field {
    pub fn named(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: ty.into(),
        }
    }

    pub fn unnamed(ty: impl Into<String>) -> Self {
        Self {
            name: String::new(),
            ty: ty.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FuncDecl {
    pub name: String,
    pub receiver: Option<Field>,
    pub params: Vec<Field>,
    pub results: Vec<Field>,
    /// Preformatted statement lines. The corpus is about declaration and
    /// signature choices; expression formatting can supply richer docs later.
    pub body: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Formatter {
    pub indent: u16,
}

impl Default for Formatter {
    fn default() -> Self {
        Self { indent: 4 }
    }
}

fn tagged(tag_id: TagId, value: impl AsRef<str>) -> Rc<Doc> {
    tag(tag_id, text(value))
}

fn keyword(value: &str) -> Rc<Doc> {
    tagged(TAG_KEYWORD, value)
}

fn ident(value: &str) -> Rc<Doc> {
    tagged(TAG_IDENT, value)
}

fn type_name(value: &str) -> Rc<Doc> {
    tagged(TAG_TYPE, value)
}

fn string_literal(value: &str) -> Rc<Doc> {
    tagged(TAG_STRING, format!("\"{}\"", value.escape_default()))
}

fn double_hardline() -> Rc<Doc> {
    concat([hardline(), hardline()])
}

fn import_parent(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(prefix, _)| prefix)
}

impl Formatter {
    pub fn format_file(&self, file: &File) -> Rc<Doc> {
        let package = concat([keyword("package"), tokens::space(), ident(&file.package)]);
        let imports = self.format_imports(&file.imports);
        let declarations = join(
            double_hardline(),
            file.declarations.iter().map(|decl| self.format_func(decl)),
        );

        let mut sections = vec![package];
        if !file.imports.is_empty() {
            sections.push(imports);
        }
        if !file.declarations.is_empty() {
            sections.push(declarations);
        }
        join(double_hardline(), sections)
    }

    fn format_import(&self, import: &Import) -> Rc<Doc> {
        let mut docs = Vec::new();
        if let Some(name) = &import.name {
            docs.push(ident(name));
            docs.push(tokens::space());
        }
        docs.push(string_literal(&import.path));
        concat(docs)
    }

    fn import_sections<'a>(&self, imports: &'a [Import]) -> Vec<Vec<&'a Import>> {
        let mut singles = Vec::new();
        let mut groups = Vec::new();
        let mut index = 0;

        while index < imports.len() {
            let current_parent = import_parent(&imports[index].path);
            let grouped = current_parent.is_some()
                && imports
                    .get(index + 1)
                    .is_some_and(|next| import_parent(&next.path) == current_parent);
            if !grouped {
                singles.push(&imports[index]);
                index += 1;
                continue;
            }

            let mut group = vec![&imports[index]];
            index += 1;
            while index < imports.len() && import_parent(&imports[index].path) == current_parent {
                group.push(&imports[index]);
                index += 1;
            }
            groups.push(group);
        }

        let mut sections = Vec::new();
        if !singles.is_empty() {
            sections.push(singles);
        }
        sections.extend(groups);
        sections
    }

    fn format_imports(&self, imports: &[Import]) -> Rc<Doc> {
        if imports.is_empty() {
            return crate::empty();
        }
        if imports.len() == 1 {
            return concat([
                keyword("import"),
                tokens::space(),
                self.format_import(&imports[0]),
            ]);
        }

        let indentation = " ".repeat(usize::from(self.indent));
        let sections = self.import_sections(imports).into_iter().map(|section| {
            join(
                hardline(),
                section.into_iter().map(|import| {
                    concat([tokens::indent(&indentation), self.format_import(import)])
                }),
            )
        });
        let body = join(double_hardline(), sections);
        concat([
            keyword("import"),
            tokens::space(),
            tokens::lparen(),
            hardline(),
            body,
            hardline(),
            tokens::rparen(),
        ])
    }

    fn format_field(&self, field: &Field) -> Rc<Doc> {
        if field.name.is_empty() {
            type_name(&field.ty)
        } else {
            concat([ident(&field.name), tokens::space(), type_name(&field.ty)])
        }
    }

    fn field_list(&self, fields: &[Field]) -> Rc<Doc> {
        let separator = concat([tokens::comma(), line()]);
        group(concat([
            tokens::lparen(),
            nest(
                self.indent,
                concat([
                    softline(),
                    join(
                        separator,
                        fields.iter().map(|field| self.format_field(field)),
                    ),
                ]),
            ),
            softline(),
            tokens::rparen(),
        ]))
    }

    fn format_results(&self, results: &[Field]) -> Rc<Doc> {
        if results.is_empty() {
            crate::empty()
        } else if results.len() == 1 && results[0].name.is_empty() {
            concat([tokens::space(), type_name(&results[0].ty)])
        } else {
            concat([tokens::space(), self.field_list(results)])
        }
    }

    pub fn format_func(&self, decl: &FuncDecl) -> Rc<Doc> {
        let receiver = decl
            .receiver
            .as_ref()
            .map_or_else(crate::empty, |receiver| {
                concat([
                    tokens::lparen(),
                    self.format_field(receiver),
                    tokens::rparen(),
                    tokens::space(),
                ])
            });
        let signature = group(concat([
            keyword("func"),
            tokens::space(),
            receiver,
            ident(&decl.name),
            self.field_list(&decl.params),
            self.format_results(&decl.results),
        ]));

        if decl.body.is_empty() {
            return concat([
                signature,
                tokens::space(),
                tokens::lbrace(),
                tokens::rbrace(),
            ]);
        }

        let body = join(
            hardline(),
            decl.body
                .iter()
                .map(|statement| tagged(TAG_STATEMENT, statement)),
        );
        group(concat([
            signature,
            tokens::space(),
            tokens::lbrace(),
            nest(self.indent, concat([line(), body])),
            line(),
            tokens::rbrace(),
        ]))
    }
}

/// The documented example from the Go prototype.
pub fn sample_file() -> File {
    File {
        package: "main".into(),
        imports: vec![
            Import::new("fmt"),
            Import::new("strings"),
            Import::named("json", "encoding/json"),
            Import::new("encoding/xml"),
        ],
        declarations: vec![FuncDecl {
            name: "processData".into(),
            receiver: Some(Field::named("p", "Processor")),
            params: vec![
                Field::named("data", "[]byte"),
                Field::named("options", "*Options"),
            ],
            results: vec![
                Field::named("result", "ProcessResult"),
                Field::named("err", "error"),
            ],
            body: vec!["// ... formatted statements".into()],
        }],
    }
}

/// A small signature where first-fit breaks the parameter list too early,
/// while the optimal engine keeps it flat and breaks only the result list.
pub fn asymmetric_file() -> File {
    File {
        package: "p".into(),
        imports: Vec::new(),
        declarations: vec![FuncDecl {
            name: "f".into(),
            receiver: None,
            params: vec![Field::named("a", "A"), Field::named("b", "B")],
            results: vec![Field::named("x", "XXXX"), Field::named("y", "YYYY")],
            body: vec!["work()".into()],
        }],
    }
}
