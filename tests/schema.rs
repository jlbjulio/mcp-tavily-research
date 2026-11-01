//! The tool list advertised to clients (`src/tools_list.json`) and the argument rules the server
//! enforces (`tavily::schema`) are written separately; this keeps them describing the same tools.

use mcp_tavily_research::json::as_js_f64;
use mcp_tavily_research::tavily::{TOOL_NAMES, schema};
use mcp_tavily_research::validate::{Default, Kind};
use serde_json::{Value, json};

fn num(v: &Value) -> Option<f64> {
    v.as_number().map(as_js_f64)
}

#[test]
fn advertised_schemas_match_the_enforced_rules() {
    let list: Value = serde_json::from_str(include_str!("../src/tools_list.json")).unwrap();
    let tools = list["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, TOOL_NAMES);

    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let input = &tool["inputSchema"];
        let props = input["properties"].as_object().unwrap();
        let fields = schema(name);
        let field_names: Vec<&str> = fields.iter().map(|f| f.name).collect();
        let prop_names: Vec<&str> = props.keys().map(String::as_str).collect();
        assert_eq!(field_names, prop_names, "{name}: properties");

        let required: Vec<&str> = input["required"].as_array().map(|r| r.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let enforced: Vec<&str> = fields.iter().filter(|f| f.required).map(|f| f.name).collect();
        assert_eq!(enforced, required, "{name}: required");

        for field in &fields {
            let p = &props[field.name];
            let at = format!("{name}.{}", field.name);
            let default = match field.default {
                None => Value::Null,
                Some(Default::Str(s)) => json!(s),
                Some(Default::Num(n)) => json!(n),
                Some(Default::Bool(b)) => json!(b),
            };
            let advertised_default = p.get("default").cloned().unwrap_or(Value::Null);
            assert!(
                default == advertised_default || num(&default).is_some_and(|d| num(&advertised_default) == Some(d)),
                "{at}: default {default} vs {advertised_default}"
            );
            match &field.kind {
                Kind::Str { min, max } => {
                    assert_eq!(p["type"], "string", "{at}");
                    assert_eq!(num(&p["minLength"]), Some(*min as f64), "{at}: minLength");
                    assert_eq!(num(&p["maxLength"]), max.map(|m| m as f64), "{at}: maxLength");
                }
                Kind::Int { min, max } | Kind::Num { min, max } => {
                    let ty = if matches!(field.kind, Kind::Int { .. }) { "integer" } else { "number" };
                    assert_eq!(p["type"], ty, "{at}");
                    assert_eq!(num(&p["minimum"]), Some(*min), "{at}: minimum");
                    assert_eq!(num(&p["maximum"]), Some(*max), "{at}: maximum");
                }
                Kind::Bool => assert_eq!(p["type"], "boolean", "{at}"),
                Kind::Enum(options) => {
                    let advertised: Vec<&str> = p["enum"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
                    assert_eq!(advertised, *options, "{at}: enum");
                }
                Kind::StrList { max } => {
                    assert_eq!(p["type"], "array", "{at}");
                    assert_eq!(num(&p["maxItems"]), Some(*max as f64), "{at}: maxItems");
                    assert_eq!(p["items"]["type"], "string", "{at}: items");
                    assert_eq!(num(&p["items"]["minLength"]), Some(1.0), "{at}: items.minLength");
                }
                Kind::WebUrl => {
                    assert_eq!(p["type"], "string", "{at}");
                    assert_eq!(p["format"], "uri", "{at}");
                }
                Kind::WebUrls => {
                    let any = p["anyOf"].as_array().unwrap();
                    assert_eq!(any[0]["format"], "uri", "{at}: single URL");
                    assert_eq!(num(&any[1]["minItems"]), Some(1.0), "{at}: minItems");
                    assert_eq!(num(&any[1]["maxItems"]), Some(20.0), "{at}: maxItems");
                    assert_eq!(any[1]["items"]["format"], "uri", "{at}: items");
                }
            }
        }
    }
}
