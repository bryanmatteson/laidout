//! SQL formatter corpus recovered from `internal/doctype/sql_test.go`.

use crate::doc::{concat, group, hardline, join, nest, tag, text, Doc, TagId};
use crate::{tags, tokens};

pub const TAG_KEYWORD: TagId = tags::CUSTOM_START + 16;
pub const TAG_EXPRESSION: TagId = tags::CUSTOM_START + 17;
pub const TAG_IDENT: TagId = tags::CUSTOM_START + 18;
pub const TAG_NUMBER: TagId = tags::CUSTOM_START + 19;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Query {
    pub select: Vec<SelectItem>,
    pub from: TableRef,
    pub joins: Vec<JoinClause>,
    pub where_clause: String,
    pub group_by: Vec<String>,
    pub having: String,
    pub order_by: Vec<OrderByItem>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub with: Vec<CteClause>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectItem {
    pub expression: String,
    pub alias: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TableRef {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JoinClause {
    pub kind: String,
    pub table: TableRef,
    pub condition: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderByItem {
    pub expression: String,
    pub direction: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CteClause {
    pub name: String,
    pub query: Query,
    pub columns: Vec<String>,
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

fn tagged(tag_id: TagId, value: impl AsRef<str>) -> Doc {
    tag(tag_id, text(value))
}

fn keyword(value: &str) -> Doc {
    tagged(TAG_KEYWORD, value.to_uppercase())
}

fn expression(value: &str) -> Doc {
    tagged(TAG_EXPRESSION, value)
}

fn ident(value: &str) -> Doc {
    tagged(TAG_IDENT, value)
}

impl Formatter {
    pub fn format_query(&self, query: &Query) -> Doc {
        concat([
            self.format_with(&query.with),
            self.format_select(&query.select),
            self.format_from(&query.from),
            self.format_joins(&query.joins),
            self.format_where(&query.where_clause),
            self.format_group_by(&query.group_by),
            self.format_having(&query.having),
            self.format_order_by(&query.order_by),
            self.format_limit_offset(query.limit, query.offset),
        ])
    }

    fn format_cte(&self, cte: &CteClause) -> Doc {
        let columns = if cte.columns.is_empty() {
            crate::empty()
        } else {
            concat([
                tokens::space(),
                tokens::lparen(),
                join(
                    concat([tokens::comma(), tokens::space()]),
                    cte.columns.iter().map(|column| ident(column)),
                ),
                tokens::rparen(),
            ])
        };
        concat([
            ident(&cte.name),
            columns,
            tokens::space(),
            keyword("as"),
            tokens::space(),
            tokens::lparen(),
            nest(
                self.indent,
                concat([hardline(), self.format_query(&cte.query)]),
            ),
            hardline(),
            tokens::rparen(),
        ])
    }

    fn format_with(&self, ctes: &[CteClause]) -> Doc {
        if ctes.is_empty() {
            return crate::empty();
        }
        concat([
            keyword("with"),
            tokens::space(),
            join(
                concat([tokens::comma(), hardline()]),
                ctes.iter().map(|cte| self.format_cte(cte)),
            ),
            hardline(),
        ])
    }

    fn format_select(&self, items: &[SelectItem]) -> Doc {
        let selections = items.iter().map(|item| {
            if let Some(alias) = &item.alias {
                group(concat([
                    expression(&item.expression),
                    tokens::space(),
                    keyword("as"),
                    tokens::space(),
                    ident(alias),
                ]))
            } else {
                expression(&item.expression)
            }
        });
        concat([
            keyword("select"),
            nest(
                self.indent,
                concat([
                    hardline(),
                    join(concat([tokens::comma(), hardline()]), selections),
                ]),
            ),
        ])
    }

    fn format_table_ref(&self, table: &TableRef) -> Doc {
        if let Some(alias) = &table.alias {
            group(concat([ident(&table.name), tokens::space(), ident(alias)]))
        } else {
            ident(&table.name)
        }
    }

    fn format_from(&self, table: &TableRef) -> Doc {
        concat([
            hardline(),
            keyword("from"),
            tokens::space(),
            self.format_table_ref(table),
        ])
    }

    fn format_joins(&self, joins: &[JoinClause]) -> Doc {
        concat(joins.iter().map(|join_clause| {
            concat([
                hardline(),
                keyword(&format!("{} join", join_clause.kind)),
                tokens::space(),
                self.format_table_ref(&join_clause.table),
                nest(
                    self.indent,
                    concat([
                        hardline(),
                        keyword("on"),
                        tokens::space(),
                        expression(&join_clause.condition),
                    ]),
                ),
            ])
        }))
    }

    fn format_condition(&self, condition: &str) -> Doc {
        join(
            concat([hardline(), keyword("and"), tokens::space()]),
            condition.split(" AND ").map(str::trim).map(expression),
        )
    }

    fn format_where(&self, condition: &str) -> Doc {
        if condition.is_empty() {
            return crate::empty();
        }
        concat([
            hardline(),
            keyword("where"),
            tokens::space(),
            nest(self.indent, self.format_condition(condition)),
        ])
    }

    fn format_group_by(&self, columns: &[String]) -> Doc {
        if columns.is_empty() {
            return crate::empty();
        }
        if columns.len() == 1 {
            return concat([
                hardline(),
                keyword("group by"),
                tokens::space(),
                expression(&columns[0]),
            ]);
        }
        concat([
            hardline(),
            keyword("group by"),
            nest(
                self.indent,
                concat([
                    hardline(),
                    join(
                        concat([tokens::comma(), hardline()]),
                        columns.iter().map(|column| expression(column)),
                    ),
                ]),
            ),
        ])
    }

    fn format_having(&self, condition: &str) -> Doc {
        if condition.is_empty() {
            crate::empty()
        } else {
            concat([
                hardline(),
                keyword("having"),
                tokens::space(),
                expression(condition),
            ])
        }
    }

    fn format_order_by(&self, items: &[OrderByItem]) -> Doc {
        if items.is_empty() {
            return crate::empty();
        }
        concat([
            hardline(),
            keyword("order by"),
            tokens::space(),
            join(
                concat([tokens::comma(), tokens::space()]),
                items.iter().map(|item| {
                    group(concat([
                        expression(&item.expression),
                        tokens::space(),
                        keyword(&item.direction),
                    ]))
                }),
            ),
        ])
    }

    fn format_limit_offset(&self, limit: Option<u64>, offset: Option<u64>) -> Doc {
        let limit = limit.map_or_else(crate::empty, |value| {
            concat([
                hardline(),
                keyword("limit"),
                tokens::space(),
                tagged(TAG_NUMBER, value.to_string()),
            ])
        });
        let offset = offset.map_or_else(crate::empty, |value| {
            concat([
                hardline(),
                keyword("offset"),
                tokens::space(),
                tagged(TAG_NUMBER, value.to_string()),
            ])
        });
        concat([limit, offset])
    }
}

fn base_query() -> Query {
    Query {
        select: vec![
            SelectItem {
                expression: "user_id".into(),
                alias: None,
            },
            SelectItem {
                expression: "COUNT(*)".into(),
                alias: Some("total_orders".into()),
            },
        ],
        from: TableRef {
            name: "orders".into(),
            alias: None,
        },
        joins: Vec::new(),
        where_clause: String::new(),
        group_by: vec!["user_id".into()],
        having: String::new(),
        order_by: Vec::new(),
        limit: None,
        offset: None,
        with: Vec::new(),
    }
}

/// The complex CTE/join/aggregation query from the Go test corpus.
pub fn complex_query() -> Query {
    Query {
        with: vec![CteClause {
            name: "order_stats".into(),
            columns: vec!["user_id".into(), "total_orders".into()],
            query: base_query(),
        }],
        select: vec![
            SelectItem {
                expression: "u.name".into(),
                alias: None,
            },
            SelectItem {
                expression: "os.total_orders".into(),
                alias: None,
            },
            SelectItem {
                expression: "COALESCE(SUM(o.amount), 0)".into(),
                alias: Some("total_spent".into()),
            },
        ],
        from: TableRef {
            name: "users".into(),
            alias: Some("u".into()),
        },
        joins: vec![
            JoinClause {
                kind: "LEFT".into(),
                table: TableRef {
                    name: "order_stats".into(),
                    alias: Some("os".into()),
                },
                condition: "os.user_id = u.id".into(),
            },
            JoinClause {
                kind: "LEFT".into(),
                table: TableRef {
                    name: "orders".into(),
                    alias: Some("o".into()),
                },
                condition: "o.user_id = u.id".into(),
            },
        ],
        where_clause: "u.status = 'active' AND o.created_at >= CURRENT_DATE - INTERVAL '1 year'"
            .into(),
        group_by: vec!["u.name".into(), "os.total_orders".into()],
        having: "COUNT(*) > 5".into(),
        order_by: vec![OrderByItem {
            expression: "total_spent".into(),
            direction: "DESC".into(),
        }],
        limit: Some(10),
        offset: None,
    }
}
