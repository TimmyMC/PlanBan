//! Command/string templating via minijinja.
//!
//! Every external command is a template rendered against a variable context
//! (§5). `{{ var }}` substitution plus `{% if %}` / `when` expression guards let
//! the whole workflow live in config rather than code.

use minijinja::Environment;
use serde::Serialize;

use crate::Result;

/// Render a template string against a context.
pub fn render(template: &str, ctx: &impl Serialize) -> Result<String> {
    let env = Environment::new();
    let tmpl = env.template_from_str(template)?;
    Ok(tmpl.render(ctx)?)
}

/// Evaluate a minijinja expression to a boolean — used for `when` step guards
/// (Constitution §2: steps may be conditional, but the condition is explicit and
/// declared in config).
pub fn eval_bool(expr: &str, ctx: &impl Serialize) -> Result<bool> {
    let env = Environment::new();
    let compiled = env.compile_expression(expr)?;
    let value = compiled.eval(ctx)?;
    Ok(value.is_true())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn substitutes_variables() {
        let ctx = json!({ "ticket": "PROJ-12", "branch": "feature/PROJ-12" });
        let out = render("git worktree add ../wt/{{ ticket }} -b {{ branch }}", &ctx).unwrap();
        assert_eq!(out, "git worktree add ../wt/PROJ-12 -b feature/PROJ-12");
    }

    #[test]
    fn conditional_block() {
        let ctx = json!({ "draft": true });
        let out = render("az repos pr create{% if draft %} --draft{% endif %}", &ctx).unwrap();
        assert_eq!(out, "az repos pr create --draft");
    }

    #[test]
    fn guard_evaluates_truthiness() {
        let ctx = json!({ "status": "In Progress" });
        assert!(eval_bool("status == 'In Progress'", &ctx).unwrap());
        assert!(!eval_bool("status == 'Done'", &ctx).unwrap());
    }
}
