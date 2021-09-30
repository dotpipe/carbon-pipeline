extern crate handlebars;
pub use handlebars::*;

use rcalc_lib::parse;
use regex::Regex;

#[macro_export]
macro_rules! register_helpers {
    ($var_name:ident) => {
        let mut $var_name = carbon_core::handlebars_helpers::Handlebars::new();
        carbon_core::handlebars_helpers::register(&mut $var_name);
    };
}

pub fn syntax_transform(syntax: &str) -> String {
    let re = Regex::new(r"\{\{(?P<script>((.|\n)*?))}}").unwrap();
    let captures = re.captures_iter(syntax);
    let mut new_syntax = syntax.to_string();

    for caps in captures {
        let to = syntax_wrapper(&caps["script"]);
        let from = caps.get(0).unwrap().as_str();
        new_syntax = new_syntax.replace(from, &to);
    }

    new_syntax
}
pub fn syntax_transform_new(syntax: &str) -> String {
    let re = Regex::new(r"\$\{(?P<script>((.|\n)*?))}").unwrap();
    let captures = re.captures_iter(syntax);
    let mut new_syntax = syntax.to_string();

    for caps in captures {
        let to = syntax_wrapper(&caps["script"]);
        let from = caps.get(0).unwrap().as_str();
        new_syntax = new_syntax.replace(from, &to);
    }

    new_syntax
}

pub fn syntax_clear_new(syntax: &str) -> String {
    let mut occurrences = Vec::new();
    let re = Regex::new(r"\$\{(?P<script>((.|\n)*?))}").unwrap();
    let captures = re.captures_iter(syntax);

    for caps in captures {
        occurrences.push(caps["script"].to_string());
    }

    if occurrences.len() > 0 {
        occurrences.join(" ").to_string()
    } else {
        syntax.to_string()
    }
}

pub fn syntax_clear(syntax: &str) -> String {
    let mut occurrences = Vec::new();
    let re = Regex::new(r"\{\{(?P<script>((.|\n)*?))}}").unwrap();
    let captures = re.captures_iter(syntax);

    for caps in captures {
        occurrences.push(caps["script"].to_string());
    }

    if occurrences.len() > 0 {
        occurrences.join(" ").to_string()
    } else {
        syntax.to_string()
    }
}

pub fn syntax_wrapper(syntax: &str) -> String {
    format!("{{{{{}}}}}", syntax)
}

#[cfg(test)]
#[test]
fn test_syntax_transform() {
    assert_eq!(syntax_transform("{{ item }}"), "{{ item }}".to_string());
}
#[cfg(test)]
#[test]
fn test_syntax_transform_between() {
    assert_eq!(
        syntax_transform("{{ a }} between {{ b }}"),
        "{{ a }} between {{ b }}".to_string()
    );
}

pub fn register(handlebars: &mut Handlebars) {
    handlebars_misc_helpers::register(handlebars);

    handlebars_helper!(hex: |v: i64| format!("0x{:x}", v));
    handlebars_helper!(add: |a: i64, b: i64| a + b);
    handlebars_helper!(subtract: |a: i64, b: i64| a - b);
    handlebars_helper!(divide: |a: i64, b: i64| a / b);
    handlebars_helper!(multiply: |a: i64, b: i64| a * b);

    handlebars.register_helper("resolve", Box::new(resolve));
    handlebars.register_helper("concat", Box::new(concat));
    handlebars.register_helper("to_string", Box::new(to_string));
    handlebars.register_helper("to_number", Box::new(ToNumber));
    handlebars.register_helper("calc", Box::new(Calc));
    handlebars.register_helper("hex", Box::new(hex));
    handlebars.register_helper("add", Box::new(add));
    handlebars.register_helper("subtract", Box::new(subtract));
    handlebars.register_helper("divide", Box::new(divide));
    handlebars.register_helper("multiply", Box::new(multiply));
    handlebars.register_helper("eq", Box::new(eq));
}

pub struct Calc;

impl HelperDef for Calc {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
        let mut expr = h
            .param(0)
            .and_then(|v| v.value().as_str())
            .unwrap_or("")
            .to_string();

        for (var, value) in h.hash().iter() {
            expr = expr.replace(var, &format!("{}", value.render()))
        }

        let mut state = parse::CalcState::new();

        match parse::eval(&expr, &mut state) {
            Ok(value) => {
                let value = value.to_string().parse::<i64>().unwrap();
                Ok(handlebars::ScopedJson::Derived(
                    handlebars::JsonValue::from(value),
                ))
            }
            Err(e) => Err(RenderError::new(&format!("{}", e))),
        }
    }
}

pub fn resolve(
    h: &Helper,
    _hb: &Handlebars,
    _cx: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    match h.param(0).unwrap().value() {
        handlebars::JsonValue::String(value) => out.write(&format!("\"{}\"", value))?,
        handlebars::JsonValue::Bool(value) => out.write(&format!("{}", value))?,
        value => out.write(&format!("{}", value))?,
    }
    Ok(())
}

pub fn concat(
    h: &Helper,
    _hb: &Handlebars,
    _cx: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let mut target = String::new();

    for value in h.params().iter() {
        target = format!("{}{}", target, value.render());
    }

    out.write(&target)?;
    Ok(())
}

pub fn to_string(
    h: &Helper,
    _hb: &Handlebars,
    _cx: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let value = h.param(0).unwrap().value();

    let result = if !value.is_string() {
        format!(r#"{}"#, value)
    } else {
        value.to_string()
    };

    out.write(&result)?;
    Ok(())
}

pub fn eq(
    h: &Helper,
    _hb: &Handlebars,
    _cx: &Context,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let left = h.param(0).unwrap().value();
    let right = h.param(1).unwrap().value();

    let result = if left == right { "true" } else { "false" };

    out.write(&format!(r#"{}"#, result))?;
    Ok(())
}

pub struct ToNumber;

impl HelperDef for ToNumber {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<ScopedJson<'reg, 'rc>, RenderError> {
        let value = h.param(0).unwrap().value().as_str().unwrap().to_string();

        if value.contains(".") {
            let value = value.to_string().parse::<f32>().unwrap();
            Ok(handlebars::ScopedJson::Derived(
                handlebars::JsonValue::from(value),
            ))
        } else {
            let value = value.to_string().parse::<i32>().unwrap();
            Ok(handlebars::ScopedJson::Derived(
                handlebars::JsonValue::from(value),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::handlebars_helpers::*;
    use serde_json::json;

    #[test]
    fn equal_match_string() {
        let handlebars = handlebars::Handlebars::new();

        let result = handlebars
            .render_template(r#"{{eq name "assis"}}"#, &json!({"name": "assis"}))
            .unwrap();

        assert_eq!(&result, &"true".to_string());
    }
}
