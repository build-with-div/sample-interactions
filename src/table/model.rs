//! The table: columns, rows, header groups, and the row sorting, column
//! sizing and column resizing features. Function names follow table-core's
//! (`column_getSize` is [`Table::column_size`], and so on); each says which
//! source it ports.

use std::rc::Rc;

use super::array_sort;
use super::object_map::ObjectMap;
use super::sort_fns::BuiltinSortFn;
use super::value::Value;

/// `Number.MAX_SAFE_INTEGER`, the default `maxSize`.
pub const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

// ------------------------------------------------------------- definitions

/// `accessorFn(originalRow, index)`.
pub type AccessorFn = Rc<dyn Fn(&Value, usize) -> Value>;
/// `sortFn(rowA, rowB, columnId)`.
pub type SortFnImpl = Rc<dyn Fn(&Table, RowId, RowId, &str) -> f64>;
/// `isMultiSortEvent(event)`.
pub type IsMultiSortEvent = Rc<dyn Fn(&SortEvent) -> bool>;
/// `getRowId(originalRow, index, parentRow)`.
pub type GetRowId = Rc<dyn Fn(&Value, usize, Option<&Row>) -> Option<String>>;
/// `getSubRows(originalRow, index)`.
pub type GetSubRows = Rc<dyn Fn(&Value, usize) -> Option<Vec<Value>>>;

/// How a column reads its value from a row.
#[derive(Clone)]
pub enum Accessor {
    /// `accessorKey`: a property name, or a dotted path into nested objects.
    Key(String),
    /// `accessorFn(originalRow, index)`.
    Fn(AccessorFn),
}

/// A custom `sortFn(rowA, rowB, columnId)`, named for diagnostics.
#[derive(Clone)]
pub struct CustomSortFn {
    pub name: String,
    pub compare: SortFnImpl,
}

/// `columnDef.sortFn`.
#[derive(Clone)]
pub enum SortFnSpec {
    /// `'auto'`, the default: chosen from the first rows' values.
    Auto,
    /// A name looked up in the registered sort functions.
    Named(String),
    Custom(CustomSortFn),
}

/// `columnDef.sortUndefined`: where rows whose value is `undefined` go.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortUndefined {
    /// `false`: `undefined` reaches the sort function like any value.
    Disabled,
    /// `'first'`: first in either direction.
    First,
    /// `'last'`: last in either direction.
    Last,
    /// `1` (the default) or `-1`: after or before, then flipped by `desc`.
    Number(f64),
}

/// A column definition. Unset fields fall back to `defaultColumn`, then to
/// the features' defaults.
#[derive(Clone, Default)]
pub struct ColumnDef {
    pub id: Option<String>,
    pub accessor: Option<Accessor>,
    /// A string header (a header function is the view's business).
    pub header: Option<String>,
    /// A group column's children.
    pub columns: Vec<ColumnDef>,
    pub size: Option<f64>,
    pub min_size: Option<f64>,
    pub max_size: Option<f64>,
    pub enable_sorting: Option<bool>,
    pub sort_fn: Option<SortFnSpec>,
    pub sort_undefined: Option<SortUndefined>,
    pub invert_sorting: Option<bool>,
    pub sort_desc_first: Option<bool>,
    pub enable_multi_sort: Option<bool>,
    pub enable_resizing: Option<bool>,
}

impl ColumnDef {
    /// `{ ...defaultColumn, ...columnDef }` (`constructColumn`).
    fn over(&self, default: &ColumnDef) -> ColumnDef {
        ColumnDef {
            id: self.id.clone().or_else(|| default.id.clone()),
            accessor: self.accessor.clone().or_else(|| default.accessor.clone()),
            header: self.header.clone().or_else(|| default.header.clone()),
            columns: self.columns.clone(),
            size: self.size.or(default.size),
            min_size: self.min_size.or(default.min_size),
            max_size: self.max_size.or(default.max_size),
            enable_sorting: self.enable_sorting.or(default.enable_sorting),
            sort_fn: self.sort_fn.clone().or_else(|| default.sort_fn.clone()),
            sort_undefined: self.sort_undefined.or(default.sort_undefined),
            invert_sorting: self.invert_sorting.or(default.invert_sorting),
            sort_desc_first: self.sort_desc_first.or(default.sort_desc_first),
            enable_multi_sort: self.enable_multi_sort.or(default.enable_multi_sort),
            enable_resizing: self.enable_resizing.or(default.enable_resizing),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnResizeMode {
    /// Sizes change when the drag ends (the default).
    #[default]
    OnEnd,
    /// Sizes change on every move.
    OnChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnResizeDirection {
    #[default]
    Ltr,
    Rtl,
}

/// The event a sort toggle handler receives.
#[derive(Debug, Clone, Copy, Default)]
pub struct SortEvent {
    pub shift_key: bool,
}

/// `{ id, desc }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSort {
    pub id: String,
    pub desc: bool,
}

/// Table options, unset fields taking table-core's defaults.
#[derive(Clone, Default)]
pub struct TableOptions {
    pub enable_sorting: Option<bool>,
    pub enable_multi_sort: Option<bool>,
    pub enable_sorting_removal: Option<bool>,
    pub enable_multi_remove: Option<bool>,
    pub max_multi_sort_col_count: Option<f64>,
    pub sort_desc_first: Option<bool>,
    /// `isMultiSortEvent`; the default reads `shiftKey`.
    pub is_multi_sort_event: Option<IsMultiSortEvent>,
    pub column_resize_mode: ColumnResizeMode,
    pub column_resize_direction: ColumnResizeDirection,
    pub enable_column_resizing: Option<bool>,
    pub default_column: ColumnDef,
    /// `getRowId(originalRow, index, parentRow)`; `None` falls back to the
    /// index path.
    pub get_row_id: Option<GetRowId>,
    /// `getSubRows(originalRow, index)`.
    pub get_sub_rows: Option<GetSubRows>,
    /// The registered sort functions (`features.sortFns`); `None` registers
    /// all six built-ins.
    pub sort_fns: Option<Vec<BuiltinSortFn>>,
    pub initial_sorting: Option<Vec<ColumnSort>>,
    pub initial_column_sizing: Option<ObjectMap<f64>>,
}

// ------------------------------------------------------------------ state

/// `columnResizing` state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ColumnResizingState {
    pub start_offset: Option<f64>,
    pub start_size: Option<f64>,
    pub delta_offset: Option<f64>,
    pub delta_percentage: Option<f64>,
    /// `false` when `None`.
    pub is_resizing_column: Option<String>,
    pub column_sizing_start: Vec<(String, f64)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableState {
    pub sorting: Vec<ColumnSort>,
    pub column_sizing: ObjectMap<f64>,
    pub column_resizing: ColumnResizingState,
}

/// A sort direction (`'asc'` or `'desc'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn name(self) -> &'static str {
        match self {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        }
    }
}

/// The sort function a column resolves to.
#[derive(Clone)]
pub enum SortFn {
    Builtin(BuiltinSortFn),
    Custom(CustomSortFn),
}

impl SortFn {
    pub fn name(&self) -> String {
        match self {
            SortFn::Builtin(builtin) => builtin.name().to_owned(),
            SortFn::Custom(custom) => format!("custom:{}", custom.name),
        }
    }
}

// --------------------------------------------------------- columns, rows

/// An index into [`Table::columns`].
pub type ColumnId = usize;
/// An index into [`Table::rows`].
pub type RowId = usize;

pub struct Column {
    pub id: String,
    pub def: ColumnDef,
    pub depth: usize,
    pub parent: Option<ColumnId>,
    pub columns: Vec<ColumnId>,
}

pub struct Row {
    pub id: String,
    pub original: Value,
    /// The index among its siblings.
    pub index: usize,
    pub depth: usize,
    pub parent_id: Option<String>,
    pub sub_rows: Vec<RowId>,
}

/// A row in a row model, with its sub-rows in the model's order.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelRow {
    pub row: RowId,
    pub sub_rows: Vec<ModelRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowModel {
    pub rows: Vec<ModelRow>,
    /// Every row, depth first.
    pub flat_rows: Vec<RowId>,
}

// ---------------------------------------------------------------- headers

/// An index into [`HeaderGroups::headers`].
pub type HeaderId = usize;

#[derive(Debug, Clone)]
pub struct Header {
    pub id: String,
    pub column: ColumnId,
    pub depth: usize,
    pub index: usize,
    pub is_placeholder: bool,
    pub placeholder_id: Option<String>,
    pub col_span: usize,
    pub row_span: usize,
    pub sub_headers: Vec<HeaderId>,
    pub header_group: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct HeaderGroup {
    pub id: String,
    pub depth: usize,
    pub headers: Vec<HeaderId>,
}

/// `table.getHeaderGroups()`: groups top to bottom, and every header they
/// (and their discarded parents) reference.
#[derive(Debug, Clone)]
pub struct HeaderGroups {
    pub groups: Vec<HeaderGroup>,
    pub headers: Vec<Header>,
}

impl HeaderGroups {
    /// The header with this id, searching the groups in order.
    pub fn find(&self, id: &str) -> Option<HeaderId> {
        self.groups
            .iter()
            .flat_map(|group| group.headers.iter().copied())
            .find(|&header| self.headers[header].id == id)
    }

    /// `header.getLeafHeaders()`: post-order, the header itself last.
    pub fn leaf_headers(&self, header: HeaderId) -> Vec<HeaderId> {
        let mut leaves = Vec::new();
        self.collect_leaf_headers(header, &mut leaves);
        leaves
    }

    fn collect_leaf_headers(&self, header: HeaderId, leaves: &mut Vec<HeaderId>) {
        for &sub in &self.headers[header].sub_headers {
            self.collect_leaf_headers(sub, leaves);
        }
        leaves.push(header);
    }
}

// ------------------------------------------------------------------ table

pub struct Table {
    pub options: TableOptions,
    pub columns: Vec<Column>,
    /// The top-level columns.
    pub top_columns: Vec<ColumnId>,
    pub rows: Vec<Row>,
    /// The core row model: rows as the data gives them.
    pub core_rows: Vec<RowId>,
    pub core_flat_rows: Vec<RowId>,
    pub state: TableState,
}

/// `Math.max` on two numbers: `NaN` wins, and +0 is above -0.
pub fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == b {
        if a.is_sign_positive() {
            a
        } else {
            b
        }
    } else if a > b {
        a
    } else {
        b
    }
}

/// `Math.min` on two numbers: `NaN` wins, and -0 is below +0.
pub fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == b {
        if a.is_sign_negative() {
            a
        } else {
            b
        }
    } else if a < b {
        a
    } else {
        b
    }
}

/// `Math.round`: to the nearest integer, halves toward +∞, keeping -0.
pub fn js_round(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let floor = x.floor();
    // Exact: x and its floor are within one unit.
    let rounded = if x - floor >= 0.5 { floor + 1.0 } else { floor };
    if rounded == 0.0 && x < 0.0 {
        -0.0
    } else {
        rounded
    }
}

impl Table {
    /// `constructTable({ features, data, columns, ...options })` with the
    /// row sorting, column sizing and column resizing features and the
    /// sorted row model.
    ///
    /// # Panics
    ///
    /// When a column has no id, accessor key or string header, as
    /// table-core throws.
    pub fn new(columns: &[ColumnDef], data: &[Value], options: TableOptions) -> Table {
        let mut table = Table {
            state: TableState {
                sorting: options.initial_sorting.clone().unwrap_or_default(),
                column_sizing: options.initial_column_sizing.clone().unwrap_or_default(),
                column_resizing: ColumnResizingState::default(),
            },
            options,
            columns: Vec::new(),
            top_columns: Vec::new(),
            rows: Vec::new(),
            core_rows: Vec::new(),
            core_flat_rows: Vec::new(),
        };
        table.top_columns = table.construct_columns(columns, None, 0);
        table.core_rows = table.access_rows(data, 0, None);
        table
    }

    /// `constructColumns` and `constructColumn`.
    fn construct_columns(
        &mut self,
        defs: &[ColumnDef],
        parent: Option<ColumnId>,
        depth: usize,
    ) -> Vec<ColumnId> {
        let mut ids = Vec::new();
        for def in defs {
            let resolved = def.over(&self.options.default_column);
            let accessor_key = match &resolved.accessor {
                Some(Accessor::Key(key)) => Some(key.clone()),
                _ => None,
            };
            let id = resolved
                .id
                .clone()
                .or_else(|| accessor_key.map(|key| key.replace('.', "_")))
                .or_else(|| resolved.header.clone())
                .expect("Columns require an id when using a non-string header");
            let column = self.columns.len();
            self.columns.push(Column {
                id,
                def: resolved,
                depth,
                parent,
                columns: Vec::new(),
            });
            let children = self.construct_columns(&def.columns, Some(column), depth + 1);
            self.columns[column].columns = children;
            ids.push(column);
        }
        ids
    }

    /// `accessRows` (`createCoreRowModel`).
    fn access_rows(&mut self, data: &[Value], depth: usize, parent: Option<RowId>) -> Vec<RowId> {
        let mut rows = Vec::new();
        for (index, original) in data.iter().enumerate() {
            let parent_row = parent.map(|p| &self.rows[p]);
            let id = self
                .options
                .get_row_id
                .as_ref()
                .and_then(|get| get(original, index, parent_row))
                .unwrap_or_else(|| match parent_row {
                    Some(parent) => format!("{}.{}", parent.id, index),
                    None => index.to_string(),
                });
            let row = self.rows.len();
            self.rows.push(Row {
                id,
                original: original.clone(),
                index,
                depth,
                parent_id: parent_row.map(|parent| parent.id.clone()),
                sub_rows: Vec::new(),
            });
            self.core_flat_rows.push(row);
            rows.push(row);
            if let Some(get_sub_rows) = self.options.get_sub_rows.clone() {
                if let Some(sub_rows) =
                    get_sub_rows(original, index).filter(|rows| !rows.is_empty())
                {
                    let children = self.access_rows(&sub_rows, depth + 1, Some(row));
                    self.rows[row].sub_rows = children;
                }
            }
        }
        rows
    }

    // ------------------------------------------------------- columns

    /// `table.getAllFlatColumns()`: pre-order.
    pub fn all_flat_columns(&self) -> Vec<ColumnId> {
        let mut flat = Vec::new();
        fn visit(table: &Table, column: ColumnId, flat: &mut Vec<ColumnId>) {
            flat.push(column);
            for &child in &table.columns[column].columns {
                visit(table, child, flat);
            }
        }
        for &column in &self.top_columns {
            visit(self, column, &mut flat);
        }
        flat
    }

    /// `column.getLeafColumns()`.
    pub fn leaf_columns(&self, column: ColumnId) -> Vec<ColumnId> {
        let children = &self.columns[column].columns;
        if children.is_empty() {
            vec![column]
        } else {
            children
                .iter()
                .flat_map(|&child| self.leaf_columns(child))
                .collect()
        }
    }

    /// `table.getAllLeafColumns()`.
    pub fn all_leaf_columns(&self) -> Vec<ColumnId> {
        self.top_columns
            .iter()
            .flat_map(|&column| self.leaf_columns(column))
            .collect()
    }

    /// `table.getColumn(id)`: the last flat column with that id.
    pub fn column(&self, id: &str) -> Option<ColumnId> {
        self.all_flat_columns()
            .into_iter()
            .rev()
            .find(|&column| self.columns[column].id == id)
    }

    /// `row.getValue(columnId)`.
    pub fn value(&self, row: RowId, column_id: &str) -> Value {
        let Some(column) = self.column(column_id) else {
            return Value::Undefined;
        };
        self.column_value(row, column)
    }

    fn column_value(&self, row: RowId, column: ColumnId) -> Value {
        let row = &self.rows[row];
        match &self.columns[column].def.accessor {
            None => Value::Undefined,
            Some(Accessor::Fn(accessor)) => accessor(&row.original, row.index),
            Some(Accessor::Key(key)) if key.contains('.') => {
                // `result?.[key]` along the path.
                let mut result = row.original.clone();
                for part in key.split('.') {
                    if result.is_nullish() {
                        return Value::Undefined;
                    }
                    result = result.get(part);
                }
                result
            }
            Some(Accessor::Key(key)) => row.original.get(key),
        }
    }

    // ------------------------------------------------------- sorting

    fn registered(&self, builtin: BuiltinSortFn) -> bool {
        self.options
            .sort_fns
            .as_ref()
            .is_none_or(|registered| registered.contains(&builtin))
    }

    fn first_rows(&self) -> &[RowId] {
        &self.core_flat_rows[..self.core_flat_rows.len().min(10)]
    }

    /// `column_getAutoSortFn`.
    pub fn auto_sort_fn(&self, column: ColumnId) -> SortFn {
        let mut name = None;
        let mut is_string = false;
        for &row in self.first_rows() {
            match self.column_value(row, column) {
                Value::Date(_) => {
                    name = Some(BuiltinSortFn::Datetime);
                    break;
                }
                Value::String(text) => {
                    is_string = true;
                    if text.bytes().any(|b| b.is_ascii_digit()) {
                        name = Some(BuiltinSortFn::Alphanumeric);
                        break;
                    }
                }
                _ => {}
            }
        }
        if name.is_none() && is_string {
            name = Some(BuiltinSortFn::Text);
        }
        if let Some(mut name) = name {
            if !self.registered(name) && name == BuiltinSortFn::Alphanumeric {
                name = BuiltinSortFn::Text;
            }
            if self.registered(name) {
                return SortFn::Builtin(name);
            }
        }
        SortFn::Builtin(BuiltinSortFn::Basic)
    }

    /// `column_getAutoSortDir`.
    pub fn auto_sort_dir(&self, column: ColumnId) -> SortDirection {
        for &row in self.first_rows() {
            let value = self.column_value(row, column);
            if value.is_nullish() {
                continue;
            }
            return if value.is_string() {
                SortDirection::Asc
            } else {
                SortDirection::Desc
            };
        }
        SortDirection::Desc
    }

    /// `column_getSortFn`.
    pub fn sort_fn(&self, column: ColumnId) -> SortFn {
        match self.columns[column]
            .def
            .sort_fn
            .clone()
            .unwrap_or(SortFnSpec::Auto)
        {
            SortFnSpec::Custom(custom) => SortFn::Custom(custom),
            SortFnSpec::Auto => self.auto_sort_fn(column),
            SortFnSpec::Named(name) => match BuiltinSortFn::from_name(&name) {
                Some(builtin) if self.registered(builtin) => SortFn::Builtin(builtin),
                _ => SortFn::Builtin(BuiltinSortFn::Basic),
            },
        }
    }

    /// `column_getCanSort`.
    pub fn can_sort(&self, column: ColumnId) -> bool {
        let column = &self.columns[column];
        column.def.enable_sorting.unwrap_or(true)
            && self.options.enable_sorting.unwrap_or(true)
            && column.def.accessor.is_some()
    }

    /// `column_getCanMultiSort`.
    pub fn can_multi_sort(&self, column: ColumnId) -> bool {
        let column = &self.columns[column];
        column
            .def
            .enable_multi_sort
            .or(self.options.enable_multi_sort)
            .unwrap_or(column.def.accessor.is_some())
    }

    /// `column_getIsSorted`.
    pub fn is_sorted(&self, column: ColumnId) -> Option<SortDirection> {
        let id = &self.columns[column].id;
        self.state
            .sorting
            .iter()
            .find(|sort| &sort.id == id)
            .map(|sort| {
                if sort.desc {
                    SortDirection::Desc
                } else {
                    SortDirection::Asc
                }
            })
    }

    /// `column_getSortIndex`: -1 when unsorted.
    pub fn sort_index(&self, column: ColumnId) -> isize {
        let id = &self.columns[column].id;
        self.state
            .sorting
            .iter()
            .position(|sort| &sort.id == id)
            .map_or(-1, |index| index as isize)
    }

    /// `column_getFirstSortDir`.
    pub fn first_sort_dir(&self, column: ColumnId) -> SortDirection {
        let desc_first = self.columns[column]
            .def
            .sort_desc_first
            .or(self.options.sort_desc_first)
            .unwrap_or_else(|| self.auto_sort_dir(column) == SortDirection::Desc);
        if desc_first {
            SortDirection::Desc
        } else {
            SortDirection::Asc
        }
    }

    /// `column_getNextSortingOrder`: `None` removes the sort.
    pub fn next_sorting_order(&self, column: ColumnId, multi: bool) -> Option<SortDirection> {
        let first = self.first_sort_dir(column);
        let Some(sorted) = self.is_sorted(column) else {
            return Some(first);
        };
        if sorted != first
            && self.options.enable_sorting_removal.unwrap_or(true)
            && (!multi || self.options.enable_multi_remove.unwrap_or(true))
        {
            return None;
        }
        Some(match sorted {
            SortDirection::Desc => SortDirection::Asc,
            SortDirection::Asc => SortDirection::Desc,
        })
    }

    /// `column_toggleSorting(column, desc, multi)`.
    pub fn toggle_sorting(&mut self, column: ColumnId, desc: Option<bool>, multi: Option<bool>) {
        let multi = multi.unwrap_or(false);
        let can_multi = self.can_multi_sort(column);
        let next = self.next_sorting_order(column, multi && can_multi);
        let id = self.columns[column].id.clone();
        let old = &self.state.sorting;
        let existing = old.iter().any(|sort| sort.id == id);
        let next_desc = desc.unwrap_or(next == Some(SortDirection::Desc));
        let multi_mode = !old.is_empty() && can_multi && multi;

        enum Action {
            Add,
            Remove,
            Toggle,
            Replace,
        }
        let mut action = match (multi_mode, existing) {
            (_, true) => Action::Toggle,
            (true, false) => Action::Add,
            (false, false) => Action::Replace,
        };
        if matches!(action, Action::Toggle) && desc.is_none() && next.is_none() {
            action = Action::Remove;
        }
        let entry = ColumnSort {
            id: id.clone(),
            desc: next_desc,
        };
        self.state.sorting = match action {
            Action::Add => {
                let mut sorting = old.clone();
                sorting.push(entry);
                let max = self
                    .options
                    .max_multi_sort_col_count
                    .unwrap_or(MAX_SAFE_INTEGER);
                // splice(0, length - max): ToIntegerOrInfinity, clamped.
                let count = (sorting.len() as f64 - max).trunc();
                let count = if count.is_nan() {
                    0.0
                } else {
                    count.clamp(0.0, sorting.len() as f64)
                };
                sorting.drain(..count as usize);
                sorting
            }
            Action::Toggle if multi_mode => old
                .iter()
                .map(|sort| {
                    if sort.id == id {
                        ColumnSort {
                            id: sort.id.clone(),
                            desc: next_desc,
                        }
                    } else {
                        sort.clone()
                    }
                })
                .collect(),
            Action::Remove if multi_mode => {
                old.iter().filter(|sort| sort.id != id).cloned().collect()
            }
            Action::Remove => Vec::new(),
            Action::Toggle | Action::Replace => vec![entry],
        };
    }

    /// `column.getToggleSortingHandler()(event)`, as a header click calls
    /// it.
    pub fn toggle_sorting_handler(&mut self, column: ColumnId, event: &SortEvent) {
        if !self.can_sort(column) {
            return;
        }
        let multi = if self.can_multi_sort(column) {
            match &self.options.is_multi_sort_event {
                Some(is_multi) => is_multi(event),
                None => event.shift_key,
            }
        } else {
            false
        };
        self.toggle_sorting(column, None, Some(multi));
    }

    /// `column_clearSorting`.
    pub fn clear_sorting(&mut self, column: ColumnId) {
        let id = self.columns[column].id.clone();
        self.state.sorting.retain(|sort| sort.id != id);
    }

    /// `table_setSorting`.
    pub fn set_sorting(&mut self, sorting: Vec<ColumnSort>) {
        self.state.sorting = sorting;
    }

    /// `table_resetSorting(defaultState)`.
    pub fn reset_sorting(&mut self, default_state: bool) {
        self.state.sorting = if default_state {
            Vec::new()
        } else {
            self.options.initial_sorting.clone().unwrap_or_default()
        };
    }

    /// The core row model as a row model.
    pub fn core_row_model(&self) -> RowModel {
        fn tree(table: &Table, rows: &[RowId]) -> Vec<ModelRow> {
            rows.iter()
                .map(|&row| ModelRow {
                    row,
                    sub_rows: tree(table, &table.rows[row].sub_rows),
                })
                .collect()
        }
        RowModel {
            rows: tree(self, &self.core_rows),
            flat_rows: self.core_flat_rows.clone(),
        }
    }

    /// `table.getSortedRowModel()` (`_createSortedRowModel`).
    pub fn sorted_row_model(&self) -> RowModel {
        if self.core_rows.is_empty() || self.state.sorting.is_empty() {
            return self.core_row_model();
        }
        struct Resolved {
            id: String,
            desc: bool,
            sort_undefined: SortUndefined,
            invert: bool,
            sort_fn: SortFn,
        }
        let mut resolved = Vec::new();
        for sort in &self.state.sorting {
            let Some(column) = self.column(&sort.id) else {
                continue;
            };
            if !self.can_sort(column) {
                continue;
            }
            let def = &self.columns[column].def;
            resolved.push(Resolved {
                id: sort.id.clone(),
                desc: sort.desc,
                sort_undefined: def.sort_undefined.unwrap_or(SortUndefined::Number(1.0)),
                invert: def.invert_sorting.unwrap_or(false),
                sort_fn: self.sort_fn(column),
            });
        }
        if resolved.is_empty() {
            return self.core_row_model();
        }

        let compare = |a: &RowId, b: &RowId| -> f64 {
            for entry in &resolved {
                let mut sort_int = 0.0;
                // `if (sortUndefined)`: false, 0 and NaN skip it.
                let active = match entry.sort_undefined {
                    SortUndefined::Disabled => false,
                    SortUndefined::Number(n) => n != 0.0 && !n.is_nan(),
                    _ => true,
                };
                if active {
                    let a_undefined = self.value(*a, &entry.id).is_undefined();
                    let b_undefined = self.value(*b, &entry.id).is_undefined();
                    if a_undefined && b_undefined {
                        continue;
                    }
                    if a_undefined || b_undefined {
                        match entry.sort_undefined {
                            SortUndefined::First => return if a_undefined { -1.0 } else { 1.0 },
                            SortUndefined::Last => return if a_undefined { 1.0 } else { -1.0 },
                            SortUndefined::Number(n) => sort_int = if a_undefined { n } else { -n },
                            SortUndefined::Disabled => unreachable!(),
                        }
                    }
                }
                if sort_int == 0.0 {
                    sort_int = match &entry.sort_fn {
                        SortFn::Builtin(builtin) => {
                            builtin.compare(&self.value(*a, &entry.id), &self.value(*b, &entry.id))
                        }
                        SortFn::Custom(custom) => (custom.compare)(self, *a, *b, &entry.id),
                    };
                }
                // `sortInt !== 0`, which NaN passes.
                if sort_int != 0.0 {
                    if entry.desc {
                        sort_int *= -1.0;
                    }
                    if entry.invert {
                        sort_int *= -1.0;
                    }
                    return sort_int;
                }
            }
            self.rows[*a].index as f64 - self.rows[*b].index as f64
        };

        let mut flat_rows = Vec::new();
        fn sort_data(
            table: &Table,
            rows: &[RowId],
            compare: &dyn Fn(&RowId, &RowId) -> f64,
            flat_rows: &mut Vec<RowId>,
        ) -> Vec<ModelRow> {
            let mut sorted = rows.to_vec();
            array_sort::sort(&mut sorted, compare);
            sorted
                .into_iter()
                .map(|row| {
                    flat_rows.push(row);
                    let sub_rows = sort_data(table, &table.rows[row].sub_rows, compare, flat_rows);
                    ModelRow { row, sub_rows }
                })
                .collect()
        }
        let rows = sort_data(self, &self.core_rows, &compare, &mut flat_rows);
        RowModel { rows, flat_rows }
    }

    // ------------------------------------------------------- headers

    /// `table.getHeaderGroups()` (`buildHeaderGroups`).
    pub fn header_groups(&self) -> HeaderGroups {
        let max_depth = self.max_header_depth(&self.top_columns, 1);
        let mut built = HeaderGroups {
            groups: Vec::new(),
            headers: Vec::new(),
        };
        let bottom: Vec<HeaderId> = self
            .all_leaf_columns()
            .into_iter()
            .enumerate()
            .map(|(index, column)| {
                built.headers.push(Header {
                    id: self.columns[column].id.clone(),
                    column,
                    depth: max_depth,
                    index,
                    is_placeholder: false,
                    placeholder_id: None,
                    col_span: 0,
                    row_span: 0,
                    sub_headers: Vec::new(),
                    header_group: None,
                });
                built.headers.len() - 1
            })
            .collect();
        self.construct_header_group(&mut built, bottom, max_depth - 1);
        built.groups.reverse();
        let group_count = built.groups.len();
        // Headers point at their group by its final position.
        for header in &mut built.headers {
            if let Some(group) = header.header_group.as_mut() {
                *group = group_count - 1 - *group;
            }
        }
        let top = built
            .groups
            .first()
            .map(|group| group.headers.clone())
            .unwrap_or_default();
        update_header_spans(&mut built, &top);
        built
    }

    /// `getMaxHeaderDepth`.
    fn max_header_depth(&self, columns: &[ColumnId], depth: usize) -> usize {
        let mut max = depth;
        for &column in columns {
            let children = &self.columns[column].columns;
            if !children.is_empty() {
                max = max.max(self.max_header_depth(children, depth + 1));
            }
        }
        max
    }

    /// `constructHeaderGroup`.
    fn construct_header_group(
        &self,
        built: &mut HeaderGroups,
        to_group: Vec<HeaderId>,
        depth: usize,
    ) {
        let group = built.groups.len();
        let mut pending: Vec<HeaderId> = Vec::new();
        for &header in &to_group {
            let header_column = built.headers[header].column;
            let is_leaf = self.columns[header_column].depth == depth;
            let (column, is_placeholder) = match self.columns[header_column].parent {
                Some(parent) if is_leaf => (parent, false),
                _ => (header_column, true),
            };
            match pending.last() {
                Some(&latest) if built.headers[latest].column == column => {
                    built.headers[latest].sub_headers.push(header);
                }
                _ => {
                    let placeholder_id = is_placeholder.then(|| {
                        pending
                            .iter()
                            .filter(|&&p| built.headers[p].column == column)
                            .count()
                            .to_string()
                    });
                    let id = format_header_id(
                        depth,
                        &self.columns[column].id,
                        &built.headers[header].id,
                    );
                    built.headers.push(Header {
                        id,
                        column,
                        depth,
                        index: pending.len(),
                        is_placeholder,
                        placeholder_id,
                        col_span: 0,
                        row_span: 0,
                        sub_headers: vec![header],
                        header_group: None,
                    });
                    pending.push(built.headers.len() - 1);
                }
            }
            built.headers[header].header_group = Some(group);
        }
        built.groups.push(HeaderGroup {
            id: depth.to_string(),
            depth,
            headers: to_group,
        });
        if depth > 0 {
            self.construct_header_group(built, pending, depth - 1);
        }
    }

    // -------------------------------------------------------- sizing

    /// `column_getSize`.
    pub fn column_size(&self, column: ColumnId) -> f64 {
        let column = &self.columns[column];
        let stored = self.state.column_sizing.get(&column.id).copied();
        js_min(
            js_max(
                column.def.min_size.unwrap_or(20.0),
                stored.or(column.def.size).unwrap_or(150.0),
            ),
            column.def.max_size.unwrap_or(MAX_SAFE_INTEGER),
        )
    }

    /// `buildColumnOffsets` over the leaf columns: (starts, afters).
    fn column_offsets(&self) -> (ObjectMap<f64>, ObjectMap<f64>) {
        let columns = self.all_leaf_columns();
        let sizes: Vec<f64> = columns
            .iter()
            .map(|&column| self.column_size(column))
            .collect();
        let mut starts = ObjectMap::new();
        let mut start = 0.0;
        for (i, &column) in columns.iter().enumerate() {
            starts.insert(&self.columns[column].id, start);
            start += sizes[i];
        }
        let mut afters = ObjectMap::new();
        let mut after = 0.0;
        for (i, &column) in columns.iter().enumerate().rev() {
            afters.insert(&self.columns[column].id, after);
            after += sizes[i];
        }
        (starts, afters)
    }

    /// `column_getStart`.
    pub fn column_start(&self, column: ColumnId) -> f64 {
        self.column_offsets()
            .0
            .get(&self.columns[column].id)
            .copied()
            .unwrap_or(0.0)
    }

    /// `column_getAfter`.
    pub fn column_after(&self, column: ColumnId) -> f64 {
        self.column_offsets()
            .1
            .get(&self.columns[column].id)
            .copied()
            .unwrap_or(0.0)
    }

    /// `header_getSize`: a leaf's column size, or its sub-headers' sum.
    pub fn header_size(&self, groups: &HeaderGroups, header: HeaderId) -> f64 {
        let header = &groups.headers[header];
        if header.sub_headers.is_empty() {
            return self.column_size(header.column);
        }
        let mut sum = 0.0;
        for &sub in &header.sub_headers {
            sum += self.header_size(groups, sub);
        }
        sum
    }

    /// `header_getStart`: the previous sibling's start plus its size.
    pub fn header_start(&self, groups: &HeaderGroups, header: HeaderId) -> f64 {
        let data = &groups.headers[header];
        if data.index > 0 {
            if let Some(group) = data.header_group {
                if let Some(&previous) = groups.groups[group].headers.get(data.index - 1) {
                    return self.header_start(groups, previous)
                        + self.header_size(groups, previous);
                }
            }
        }
        0.0
    }

    /// `table_getTotalSize`: the top header group's sizes, summed.
    pub fn total_size(&self, groups: &HeaderGroups) -> f64 {
        groups.groups.first().map_or(0.0, |group| {
            group
                .headers
                .iter()
                .fold(0.0, |sum, &header| sum + self.header_size(groups, header))
        })
    }

    /// `table_setColumnSizing`.
    pub fn set_column_sizing(&mut self, sizing: ObjectMap<f64>) {
        self.state.column_sizing = sizing;
    }

    /// `column_resetSize`.
    pub fn reset_size(&mut self, column: ColumnId) {
        let id = &self.columns[column].id;
        let rest = ObjectMap::from_entries(
            self.state
                .column_sizing
                .entries()
                .into_iter()
                .filter(|(key, _)| key != id),
        );
        self.state.column_sizing = rest;
    }

    /// `table_resetColumnSizing(defaultState)`.
    pub fn reset_column_sizing(&mut self, default_state: bool) {
        self.state.column_sizing = if default_state {
            ObjectMap::new()
        } else {
            self.options
                .initial_column_sizing
                .clone()
                .unwrap_or_default()
        };
    }

    // ------------------------------------------------------ resizing

    /// `column_getCanResize`.
    pub fn can_resize(&self, column: ColumnId) -> bool {
        self.columns[column].def.enable_resizing.unwrap_or(true)
            && self.options.enable_column_resizing.unwrap_or(true)
    }

    /// `column_getIsResizing`.
    pub fn is_resizing(&self, column: ColumnId) -> bool {
        self.state.column_resizing.is_resizing_column.as_deref()
            == Some(self.columns[column].id.as_str())
    }

    /// `header.getResizeHandler()(event)` for a mousedown or touchstart on
    /// `header`'s resizer. Returns the drag, which the document's move and
    /// end events drive; `None` when the column can't resize or a touch
    /// starts with more than one finger.
    pub fn begin_resize(
        &mut self,
        groups: &HeaderGroups,
        header: HeaderId,
        event: ResizeStart,
    ) -> Option<ResizeSession> {
        let column = self.column(&self.columns[groups.headers[header].column].id)?;
        if !self.can_resize(column) {
            return None;
        }
        if let ResizeStart::Touch { touches } = &event {
            if touches.len() > 1 {
                return None;
            }
        }
        let start_size = self.header_size(groups, header);
        let column_sizing_start = groups
            .leaf_headers(header)
            .into_iter()
            .map(|leaf| {
                let leaf_column = groups.headers[leaf].column;
                (
                    self.columns[leaf_column].id.clone(),
                    self.column_size(leaf_column),
                )
            })
            .collect();
        let (client_x, touch) = match event {
            ResizeStart::Mouse { client_x } => (client_x, false),
            ResizeStart::Touch { touches } => (js_round(touches[0]), true),
        };
        self.state.column_resizing = ColumnResizingState {
            start_offset: Some(client_x),
            start_size: Some(start_size),
            delta_offset: Some(0.0),
            delta_percentage: Some(0.0),
            is_resizing_column: Some(self.columns[column].id.clone()),
            column_sizing_start,
        };
        Some(ResizeSession {
            touch,
            new_column_sizing: ObjectMap::new(),
            latest_move_x: None,
            frame_requested: false,
            has_pending_move: false,
        })
    }

    /// `updateOffset(eventType, clientXPos)`.
    fn update_offset(&mut self, session: &mut ResizeSession, end: bool, client_x: Option<f64>) {
        let Some(client_x) = client_x else {
            return;
        };
        let commit = self.options.column_resize_mode == ColumnResizeMode::OnChange || end;
        let old = &self.state.column_resizing;
        let direction = if self.options.column_resize_direction == ColumnResizeDirection::Rtl {
            -1.0
        } else {
            1.0
        };
        let delta_offset = (client_x - old.start_offset.unwrap_or(0.0)) * direction;
        let start_size = old.start_size.unwrap_or(0.0);
        let delta_percentage = js_max(
            if start_size > 0.0 {
                delta_offset / start_size
            } else {
                0.0
            },
            -0.999999,
        );
        if commit {
            let count = old.column_sizing_start.len() as f64;
            for (id, header_size) in &old.column_sizing_start {
                let size = if *header_size > 0.0 {
                    header_size + header_size * delta_percentage
                } else {
                    delta_offset / count
                };
                session
                    .new_column_sizing
                    .insert(id, js_round(js_max(size, 0.0) * 100.0) / 100.0);
            }
        }
        self.state.column_resizing.delta_offset = Some(delta_offset);
        self.state.column_resizing.delta_percentage = Some(delta_percentage);
        if commit {
            let mut sizing = ObjectMap::new();
            sizing.assign(&self.state.column_sizing);
            sizing.assign(&session.new_column_sizing);
            self.state.column_sizing = sizing;
        }
    }
}

/// `formatHeaderId` (the table has no header family without pinning).
fn format_header_id(depth: usize, column_id: &str, child_header_id: &str) -> String {
    let mut id = String::new();
    if depth != 0 {
        id = depth.to_string();
    }
    for part in [column_id, child_header_id] {
        if !part.is_empty() {
            id = if id.is_empty() {
                part.to_owned()
            } else {
                format!("{id}_{part}")
            };
        }
    }
    id
}

/// `updateHeaderSpans`.
fn update_header_spans(built: &mut HeaderGroups, headers: &[HeaderId]) {
    for &header in headers {
        let subs = built.headers[header].sub_headers.clone();
        let col_span = if subs.is_empty() {
            1
        } else {
            update_header_spans(built, &subs);
            subs.iter().map(|&sub| built.headers[sub].col_span).sum()
        };
        built.headers[header].col_span = col_span;
        let column = built.headers[header].column;
        let chains = |built: &HeaderGroups, h: HeaderId| {
            let subs = &built.headers[h].sub_headers;
            (subs.len() == 1 && built.headers[subs[0]].column == column).then(|| subs[0])
        };
        if built.headers[header].is_placeholder && chains(built, header).is_some() {
            let mut row_span = 1;
            let mut chain = chains(built, header);
            while let Some(child) = chain {
                built.headers[child].row_span = 0;
                row_span += 1;
                chain = chains(built, child);
            }
            built.headers[header].row_span = row_span;
        } else {
            built.headers[header].row_span = 1;
        }
    }
}

/// The event that starts a column resize.
#[derive(Debug, Clone)]
pub enum ResizeStart {
    /// A mousedown at `clientX`.
    Mouse { client_x: f64 },
    /// A touchstart: each touch's `clientX`.
    Touch { touches: Vec<f64> },
}

/// What the event layer should do after a resize step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameRequest {
    None,
    /// `requestAnimationFrame(flushMove)`.
    Request,
    /// `cancelAnimationFrame(moveRafId)`.
    Cancel,
}

/// A column resize in progress: the state table-core's resize handler keeps
/// in its closure. The view listens for the document's moves and the end
/// (mouse or touch, as [`ResizeSession::touch`] says) and forwards them.
#[derive(Debug, Clone)]
pub struct ResizeSession {
    /// Started by touch: listen for `touchmove`, `touchend` and
    /// `touchcancel` instead of `mousemove` and `mouseup`.
    pub touch: bool,
    new_column_sizing: ObjectMap<f64>,
    latest_move_x: Option<f64>,
    /// `moveRafId !== null`.
    frame_requested: bool,
    has_pending_move: bool,
}

impl ResizeSession {
    /// `onMove(clientX)`. With `frames`, moves are coalesced to one per
    /// animation frame, as in a browser; without (`typeof
    /// requestAnimationFrame !== 'function'`), each applies at once.
    pub fn on_move(&mut self, table: &mut Table, client_x: f64, frames: bool) -> FrameRequest {
        self.latest_move_x = Some(client_x);
        if !frames {
            table.update_offset(self, false, Some(client_x));
            return FrameRequest::None;
        }
        if self.frame_requested {
            self.has_pending_move = true;
            return FrameRequest::None;
        }
        table.update_offset(self, false, Some(client_x));
        self.frame_requested = true;
        FrameRequest::Request
    }

    /// `flushMove`, when the requested frame arrives.
    pub fn on_frame(&mut self, table: &mut Table) -> FrameRequest {
        if self.has_pending_move {
            self.has_pending_move = false;
            let x = self.latest_move_x;
            table.update_offset(self, false, x);
            FrameRequest::Request
        } else {
            self.frame_requested = false;
            FrameRequest::None
        }
    }

    /// `onEnd(clientX)`: a mouseup's `clientX`, a touchend's first remaining
    /// touch, or `None` (touchcancel, or a touchend with no touches left),
    /// which falls back to the last move.
    pub fn on_end(mut self, table: &mut Table, client_x: Option<f64>) -> FrameRequest {
        let request = if self.frame_requested {
            FrameRequest::Cancel
        } else {
            FrameRequest::None
        };
        self.frame_requested = false;
        self.has_pending_move = false;
        let x = client_x.or(self.latest_move_x);
        table.update_offset(&mut self, true, x);
        let resizing = &mut table.state.column_resizing;
        resizing.is_resizing_column = None;
        resizing.start_offset = None;
        resizing.start_size = None;
        resizing.delta_offset = None;
        resizing.delta_percentage = None;
        resizing.column_sizing_start = Vec::new();
        request
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_round_rounds_halves_up() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert!(js_round(-0.4).is_sign_negative());
        assert_eq!(js_round(0.49999999999999994), 0.0);
    }

    #[test]
    fn coalesces_moves_to_frames() {
        let columns = [ColumnDef {
            accessor: Some(Accessor::Key("a".into())),
            ..ColumnDef::default()
        }];
        let options = TableOptions {
            column_resize_mode: ColumnResizeMode::OnChange,
            ..TableOptions::default()
        };
        let mut table = Table::new(&columns, &[], options);
        let groups = table.header_groups();
        let mut session = table
            .begin_resize(&groups, 0, ResizeStart::Mouse { client_x: 100.0 })
            .unwrap();
        assert_eq!(
            session.on_move(&mut table, 110.0, true),
            FrameRequest::Request
        );
        assert_eq!(table.state.column_sizing.get("a"), Some(&160.0));
        assert_eq!(session.on_move(&mut table, 120.0, true), FrameRequest::None);
        assert_eq!(table.state.column_sizing.get("a"), Some(&160.0));
        assert_eq!(session.on_frame(&mut table), FrameRequest::Request);
        assert_eq!(table.state.column_sizing.get("a"), Some(&170.0));
        assert_eq!(session.on_frame(&mut table), FrameRequest::None);
        assert_eq!(session.on_end(&mut table, Some(130.0)), FrameRequest::None);
        assert_eq!(table.state.column_sizing.get("a"), Some(&180.0));
        assert_eq!(table.state.column_resizing, ColumnResizingState::default());
    }
}
