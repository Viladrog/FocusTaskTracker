pub fn normalize_task_title(title: &str) -> Result<&str, String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        Err("title is empty".to_string())
    } else {
        Ok(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_create_rejects_empty_title() {
        assert_eq!(normalize_task_title(""), Err("title is empty".to_string()));
        assert_eq!(
            normalize_task_title("   "),
            Err("title is empty".to_string())
        );
    }

    #[test]
    fn task_create_trims_title() {
        assert_eq!(normalize_task_title("  hi  "), Ok("hi"));
    }
}
