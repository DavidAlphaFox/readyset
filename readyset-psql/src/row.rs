use std::sync::Arc;

use readyset_data::DfValue;
use tokio_postgres::types::Type;

use crate::value::Value;

/// A structure containing a `Vec<DfValue>`, representing one row of data, which facilitates
/// iteration over the values within this row as `Value` structures.
pub struct Row {
    /// The values comprising the row, as returned from a ReadySet interface lookup. Only the
    /// indices within this vector listed in `project_fields` will actually be projected during
    /// iteration. (See documentaion below for `project_fields`).
    pub values: Vec<DfValue>,

    /// The fields to project. A `Vec<DfValue>` returned from a ReadySet interface lookup may
    /// contain extraneous fields that should not be projected into the query result output. In
    /// particular, bogokeys and other lookup keys that are not requested for projection by the SQL
    /// query may be present in `values` but should be excluded from query output. This
    /// `project_fields` attribute contains the indices within `values` that _should_ be projected
    /// into the output.
    pub project_fields: Arc<Vec<usize>>,

    /// The data types of the projected fields for this row.
    pub project_field_types: Arc<Vec<Type>>,
}

impl IntoIterator for Row {
    type Item = Value;
    type IntoIter = RowIterator;

    fn into_iter(self) -> Self::IntoIter {
        RowIterator { row: self, pos: 0 }
    }
}

/// An iterator over a `Row`'s values. Only those field values listed in the `Row`'s
/// `project_fields` list are included in the iterator output.
pub struct RowIterator {
    /// The row being iterated.
    row: Row,

    /// The iteration position.
    pos: usize,
}

impl Iterator for RowIterator {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        let col_type = self.row.project_field_types.get(self.pos)?.clone();
        let i = *self.row.project_fields.get(self.pos)?;
        let value = self.row.values.get(i)?.clone();
        self.pos += 1;
        Some(Value { col_type, value })
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use psql_srv as ps;
    use rust_decimal::Decimal;

    use super::*;

    fn collect_row_values(row: Row) -> Vec<ps::Value> {
        row.into_iter()
            .map(|v| ps::Value::try_from(v).unwrap())
            .collect::<Vec<ps::Value>>()
    }

    #[test]
    fn iterate_empty_row() {
        let row = Row {
            values: vec![],
            project_fields: Arc::new(vec![]),
            project_field_types: Arc::new(vec![]),
        };
        assert_eq!(collect_row_values(row), Vec::<ps::Value>::new());
    }

    #[test]
    fn iterate_singleton_row() {
        let row = Row {
            values: vec![DfValue::Int(43)],
            project_fields: Arc::new(vec![0]),
            project_field_types: Arc::new(vec![Type::INT4]),
        };
        assert_eq!(collect_row_values(row), vec![ps::Value::Int(43)]);
    }

    #[test]
    fn iterate_row() {
        let row = Row {
            values: vec![
                DfValue::Int(43),
                DfValue::Text("abcde".into()),
                DfValue::Double(10.000000222),
                DfValue::Float(8.99),
                DfValue::from(Decimal::new(35901234, 4)), // 3590.1234
            ],
            project_fields: Arc::new(vec![0, 1, 2, 3, 4]),
            project_field_types: Arc::new(vec![
                Type::INT4,
                Type::TEXT,
                Type::FLOAT8,
                Type::FLOAT4,
                Type::NUMERIC,
            ]),
        };
        assert_eq!(
            collect_row_values(row),
            vec![
                ps::Value::Int(43),
                ps::Value::Text("abcde".into()),
                ps::Value::Double(10.000000222),
                ps::Value::Float(8.99),
                ps::Value::Numeric(Decimal::new(35901234, 4)),
            ]
        );
    }

    #[test]
    fn iterate_row_with_trailing_unprojected_fields() {
        let row = Row {
            values: vec![
                DfValue::Int(43),
                DfValue::Text("abcde".into()),
                DfValue::Double(10.000000222),
                DfValue::Float(8.99),
                DfValue::from(Decimal::new(35901234, 4)), // 3590.1234
                DfValue::Int(0),
            ],
            // Only the first three fields are specified for projection.
            project_fields: Arc::new(vec![0, 1, 2, 3, 4]),
            project_field_types: Arc::new(vec![
                Type::INT4,
                Type::TEXT,
                Type::FLOAT8,
                Type::FLOAT4,
                Type::NUMERIC,
            ]),
        };
        assert_eq!(
            collect_row_values(row),
            vec![
                ps::Value::Int(43),
                ps::Value::Text("abcde".into()),
                ps::Value::Double(10.000000222),
                ps::Value::Float(8.99),
                ps::Value::Numeric(Decimal::new(35901234, 4)),
            ]
        );
    }

    #[test]
    fn iterate_row_with_interleaved_unprojected_fields() {
        let row = Row {
            values: vec![
                DfValue::Int(0),
                DfValue::Int(43),
                DfValue::Text("abcde".into()),
                DfValue::Int(0),
                DfValue::Int(0),
                DfValue::Double(10.000000222),
                DfValue::Float(8.99),
                DfValue::from(Decimal::new(35901234, 4)), // 3590.1234
                DfValue::Int(0),
            ],
            // Only some of the fields are specified for projection.
            project_fields: Arc::new(vec![1, 2, 5, 6, 7]),
            project_field_types: Arc::new(vec![
                Type::INT4,
                Type::TEXT,
                Type::FLOAT8,
                Type::FLOAT4,
                Type::NUMERIC,
            ]),
        };
        assert_eq!(
            collect_row_values(row),
            vec![
                ps::Value::Int(43),
                ps::Value::Text("abcde".into()),
                ps::Value::Double(10.000000222),
                ps::Value::Float(8.99),
                ps::Value::Numeric(Decimal::new(35901234, 4)),
            ]
        );
    }
}
