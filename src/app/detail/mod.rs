mod access;
mod delete;
mod input;
mod open;
mod save;
mod text;
mod values;

pub(crate) use text::detail_value_text;

#[cfg(test)]
pub(crate) use text::wrapped_line_count;

#[cfg(test)]
mod tests;
