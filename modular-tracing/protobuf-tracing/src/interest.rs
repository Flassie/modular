pub struct Interest<'a> {
    pub target: &'a str,
    pub parent_span_name: Option<&'a str>,
}
