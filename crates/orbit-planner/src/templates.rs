use orbit_core::plan::PlanNodeType;

static CODE: &str = include_str!("templates/code.md");
static TEST: &str = include_str!("templates/test.md");
static REVIEW: &str = include_str!("templates/review.md");
static VERIFY: &str = include_str!("templates/verify.md");
static PR: &str = include_str!("templates/pr.md");

/// Return the specialist instruction template for `task_type`, or `None` for Custom nodes.
pub fn get_template(task_type: &PlanNodeType) -> Option<&'static str> {
    match task_type {
        PlanNodeType::Code => Some(CODE),
        PlanNodeType::Test => Some(TEST),
        PlanNodeType::Review => Some(REVIEW),
        PlanNodeType::Verify => Some(VERIFY),
        PlanNodeType::Pr => Some(PR),
        PlanNodeType::Custom(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtin_types_have_templates() {
        for ty in [
            PlanNodeType::Code,
            PlanNodeType::Test,
            PlanNodeType::Review,
            PlanNodeType::Verify,
            PlanNodeType::Pr,
        ] {
            assert!(get_template(&ty).is_some(), "{ty:?} has no template");
        }
    }

    #[test]
    fn custom_type_returns_none() {
        assert!(get_template(&PlanNodeType::Custom("deploy".into())).is_none());
    }

    #[test]
    fn templates_are_non_empty() {
        for ty in [
            PlanNodeType::Code,
            PlanNodeType::Test,
            PlanNodeType::Review,
            PlanNodeType::Verify,
            PlanNodeType::Pr,
        ] {
            let t = get_template(&ty).unwrap();
            assert!(!t.trim().is_empty(), "{ty:?} template is empty");
        }
    }
}
