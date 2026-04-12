// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Action Handler
//!
//! Reads user input from stdin and maps it to core UserAction values.

use std::io::{self, BufRead, Write};

use vauchi_app::ui::{Component, ScreenAction, ScreenModel, UserAction};

/// Prompts the user based on the current screen and returns the resulting action.
///
/// The interaction priority is:
/// 1. If there is a TextInput component, prompt for text input first.
/// 2. If there is a ToggleList component, prompt for toggle selection.
/// 3. Prompt for an action button selection.
pub fn prompt_for_action(screen: &ScreenModel) -> io::Result<UserAction> {
    // Check for text input components that need filling
    for component in &screen.components {
        if let Component::TextInput {
            id, label, value, ..
        } = component
            && value.is_empty()
        {
            let input = prompt_text(label)?;
            return Ok(UserAction::TextChanged {
                component_id: id.clone(),
                value: input,
            });
        }
    }

    // Check for toggle lists
    for component in &screen.components {
        if let Component::ToggleList { id, items, .. } = component
            && !items.is_empty()
        {
            return prompt_toggle(id, items.len());
        }
    }

    // Default: prompt for action selection
    prompt_action_selection(&screen.actions)
}

/// Prompts the user for text input.
fn prompt_text(label: &str) -> io::Result<String> {
    print!("  {} > ", label);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// Prompts the user to toggle an item or proceed.
fn prompt_toggle(component_id: &str, item_count: usize) -> io::Result<UserAction> {
    loop {
        print!(
            "  Toggle item (1-{}) or press Enter to continue > ",
            item_count
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let trimmed = input.trim();

        // Empty input means "continue" — select the first action
        if trimmed.is_empty() {
            return Ok(UserAction::ActionPressed {
                action_id: "continue".into(),
            });
        }

        // Try to parse as a number for toggle
        if let Ok(num) = trimmed.parse::<usize>()
            && num >= 1
            && num <= item_count
        {
            // We need the item_id — the caller passes the screen which has the items.
            // Since we only have the count here, we construct a placeholder.
            // The onboarding command will need to look up the actual item_id.
            return Ok(UserAction::ItemToggled {
                component_id: component_id.to_string(),
                item_id: format!("__index_{}", num - 1),
            });
        }

        // Unrecognized or out-of-range input — re-prompt
        println!(
            "  Invalid input. Enter a number 1-{} or press Enter.",
            item_count
        );
    }
}

/// Prompts the user to select an action from the list.
fn prompt_action_selection(actions: &[ScreenAction]) -> io::Result<UserAction> {
    if actions.is_empty() {
        // No actions available, wait for Enter
        print!("  Press Enter to continue > ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        return Ok(UserAction::ActionPressed {
            action_id: "continue".into(),
        });
    }

    loop {
        if actions.len() == 1 {
            print!("  Press Enter for '{}' > ", actions[0].label);
        } else {
            print!("  Choose (1-{}) > ", actions.len());
        }
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        let trimmed = input.trim();

        // Empty input selects the first (primary) action
        if trimmed.is_empty() {
            return Ok(UserAction::ActionPressed {
                action_id: actions[0].id.clone(),
            });
        }

        // Try to parse as number
        if let Ok(num) = trimmed.parse::<usize>()
            && num >= 1
            && num <= actions.len()
        {
            let action = &actions[num - 1];
            if action.enabled {
                return Ok(UserAction::ActionPressed {
                    action_id: action.id.clone(),
                });
            }
            // Disabled action — re-prompt
            println!("  That action is disabled. Choose another.");
            continue;
        }

        // Invalid or out-of-range input — re-prompt
        println!(
            "  Invalid input. Enter a number 1-{} or press Enter.",
            actions.len()
        );
    }
}

/// Resolves a toggle action with placeholder item_id to the real item_id.
///
/// The `prompt_toggle` function returns `__index_N` as item_id because it does
/// not have access to the actual item data. This function resolves it using the
/// current screen's toggle list.
pub fn resolve_toggle_item_id(action: UserAction, screen: &ScreenModel) -> UserAction {
    match &action {
        UserAction::ItemToggled {
            component_id,
            item_id,
        } if item_id.starts_with("__index_") => {
            if let Some(idx) = item_id
                .strip_prefix("__index_")
                .and_then(|s| s.parse::<usize>().ok())
            {
                // Find the toggle list component
                for component in &screen.components {
                    if let Component::ToggleList { id, items, .. } = component
                        && id == component_id
                        && let Some(item) = items.get(idx)
                    {
                        return UserAction::ItemToggled {
                            component_id: component_id.clone(),
                            item_id: item.id.clone(),
                        };
                    }
                }
            }
            action
        }
        _ => action,
    }
}

// INLINE_TEST_REQUIRED: Tests need access to private resolve_toggle_item_id internals
// and the CLI is a binary crate without lib.rs
#[cfg(test)]
mod tests {
    use super::*;
    use vauchi_app::ui::ToggleItem;

    #[test]
    fn resolve_toggle_item_id_replaces_placeholder() {
        let screen = ScreenModel {
            screen_id: "test".into(),
            title: "Test".into(),
            subtitle: None,
            components: vec![Component::ToggleList {
                id: "groups".into(),
                label: "Groups".into(),
                items: vec![
                    ToggleItem {
                        id: "Family".into(),
                        label: "Family".into(),
                        selected: false,
                        subtitle: None,
                        a11y: None,
                    },
                    ToggleItem {
                        id: "Friends".into(),
                        label: "Friends".into(),
                        selected: false,
                        subtitle: None,
                        a11y: None,
                    },
                ],
                a11y: None,
            }],
            actions: vec![],
            progress: None,
            ..Default::default()
        };

        let action = UserAction::ItemToggled {
            component_id: "groups".into(),
            item_id: "__index_1".into(),
        };

        let resolved = resolve_toggle_item_id(action, &screen);
        match resolved {
            UserAction::ItemToggled {
                component_id,
                item_id,
            } => {
                assert_eq!(component_id, "groups");
                assert_eq!(item_id, "Friends");
            }
            other => panic!("Expected ItemToggled, got {:?}", other),
        }
    }

    #[test]
    fn resolve_toggle_item_id_passes_through_normal_action() {
        let screen = ScreenModel {
            screen_id: "test".into(),
            title: "Test".into(),
            subtitle: None,
            components: vec![],
            actions: vec![],
            progress: None,
            ..Default::default()
        };

        let action = UserAction::ActionPressed {
            action_id: "continue".into(),
        };

        let resolved = resolve_toggle_item_id(action, &screen);
        match resolved {
            UserAction::ActionPressed { action_id } => {
                assert_eq!(action_id, "continue");
            }
            other => panic!("Expected ActionPressed, got {:?}", other),
        }
    }

    #[test]
    fn resolve_toggle_item_id_keeps_placeholder_if_index_out_of_bounds() {
        let screen = ScreenModel {
            screen_id: "test".into(),
            title: "Test".into(),
            subtitle: None,
            components: vec![Component::ToggleList {
                id: "groups".into(),
                label: "Groups".into(),
                items: vec![ToggleItem {
                    id: "Family".into(),
                    label: "Family".into(),
                    selected: false,
                    subtitle: None,
                    a11y: None,
                }],
                a11y: None,
            }],
            actions: vec![],
            progress: None,
            ..Default::default()
        };

        let action = UserAction::ItemToggled {
            component_id: "groups".into(),
            item_id: "__index_5".into(),
        };

        let resolved = resolve_toggle_item_id(action, &screen);
        match resolved {
            UserAction::ItemToggled { item_id, .. } => {
                assert_eq!(item_id, "__index_5");
            }
            other => panic!("Expected ItemToggled, got {:?}", other),
        }
    }

    #[test]
    fn resolve_toggle_keeps_real_item_id_unchanged() {
        let screen = ScreenModel {
            screen_id: "test".into(),
            title: "Test".into(),
            subtitle: None,
            components: vec![Component::ToggleList {
                id: "groups".into(),
                label: "Groups".into(),
                items: vec![ToggleItem {
                    id: "Family".into(),
                    label: "Family".into(),
                    selected: false,
                    subtitle: None,
                    a11y: None,
                }],
                a11y: None,
            }],
            actions: vec![],
            progress: None,
            ..Default::default()
        };

        let action = UserAction::ItemToggled {
            component_id: "groups".into(),
            item_id: "Family".into(),
        };

        let resolved = resolve_toggle_item_id(action, &screen);
        match resolved {
            UserAction::ItemToggled { item_id, .. } => {
                assert_eq!(item_id, "Family");
            }
            other => panic!("Expected ItemToggled, got {:?}", other),
        }
    }
}
