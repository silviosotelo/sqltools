// SPDX-License-Identifier: GPL-3.0-only
//! Command registry built from the original Windows `ui/spec/menu_spec.json`.
//! It lists all 235 original commands and flags the ones this app implements.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSpec {
    pub id: String,
    pub name: String,
    pub keys: Vec<String>,
    pub help: String,
    pub implemented: bool,
}

/// Names handled by the Rust frontend. Keeping this explicit avoids claiming
/// parity for commands that only exist in the Windows original.
const IMPLEMENTED: &[&str] = &[
    "Edit.Undo",
    "Edit.Redo",
    "Edit.SelectAll",
    "Edit.Cut",
    "Edit.Copy",
    "Edit.Paste",
    "Edit.Uppercase",
    "Edit.Lowercase",
    "Edit.CommentLine",
    "Edit.UncommentLine",
    "Edit.InvertCase",
    "Edit.Capitalize",
    "Edit.SelectLine",
    "Edit.DeleteLine",
    "Edit.DupLineOrSelection",
    "Edit.JoinLines",
    "Edit.IndentSelection",
    "Edit.UndentSelection",
    "Edit.Find",
    "Edit.Replace",
    "Edit.FindNext",
    "Edit.FindPrevious",
    "Edit.ExpandAllFolds",
    "Edit.CollapseAllFolds",
    "Edit.TrimTrailingSpaces",
    "Edit.RemoveBlankLines",
    "Edit.Sort",
    "Edit.ToggleBookmark",
    "Edit.NextBookmark",
    "Edit.PreviousBookmark",
    "Edit.FormatSQL",
    "Edit.LineNumbers",
    "Edit.FileFormat_Winows",
    "Edit.FileFormat_Unix",
    "Edit.FileFormat_Mac",
    "Edit.FindMatch",
    "Edit.FindMatchSelect",
    "Edit.CollapseToLevel1",
    "Edit.CollapseToLevel2",
    "Edit.CollapseToLevel3",
    "Edit.SyntaxGutter",
    "Edit.Ruler",
    "Edit.ColumnMarkers",
    "Edit.FindInFile",
    "Edit.OpenFileUnderCursor",
    "Edit.VisibleSpaces",
    "Edit.IndentGuide",
    "Edit.CreateTemplate",
    "Edit.ExpandTemplate",
    "Edit.FileSettings",
    "Edit.PermanentSettings",
    "Edit.ColumnLeftJustify",
    "Edit.ColumnRightJustify",
    "Edit.ColumnCenterJustify",
    "Edit.ColumnInsert",
    "Edit.ColumnInsertNumber",
    "Edit.ColumnSelection",
    "Edit.StreamSelection",
    "Edit.ToggleSelectionMode",
    "Workspace.SaveQuick",
    "Workspace.SaveAs",
    "Workspace.OpenAutosaved",
    "Workspace.Copy",
    "Workspace.Paste",
    "Workspace.OpenClose",
    "Edit.ToggleAllFolds",
    "Edit.CollapseCurrentRegion",
    "Edit.ExpandCurrentRegion",
    "Edit.ToggleCurrentRegion",
    "Edit.ExpandCurrentSubtree",
    "Script.ExecuteInSQLPlus",
    "File.Print",
    "File.PrintPreview",
    "File.PrintSetup",
    "File.PageSetup",
    "Tools.TableTransformationHelper",
    "Edit.Autocomplete",
    "Edit.ToggleFoldingEnabled",
    "Grid.ShowPopupViewer",
    "Help.Help",
    "Help.ContextHelp",
    "Edit.SelectWord",
    "Edit.CopyAppend",
    "Edit.CutAppend",
    "Edit.Delete",
    "Edit.DeleteWordBack",
    "Edit.DeletetoEndOfWord",
    "Edit.ScrollUp",
    "Edit.ScrollDown",
    "Edit.ScrollToCenter",
    "Edit.FindSelectedNext",
    "Edit.FindSelectedPrevious",
    "Edit.SetRandomBookmark.0",
    "Edit.SetRandomBookmark.1",
    "Edit.SetRandomBookmark.2",
    "Edit.SetRandomBookmark.3",
    "Edit.SetRandomBookmark.4",
    "Edit.SetRandomBookmark.5",
    "Edit.SetRandomBookmark.6",
    "Edit.SetRandomBookmark.7",
    "Edit.SetRandomBookmark.8",
    "Edit.SetRandomBookmark.9",
    "Edit.GetRandomBookmark.0",
    "Edit.GetRandomBookmark.1",
    "Edit.GetRandomBookmark.2",
    "Edit.GetRandomBookmark.3",
    "Edit.GetRandomBookmark.4",
    "Edit.GetRandomBookmark.5",
    "Edit.GetRandomBookmark.6",
    "Edit.GetRandomBookmark.7",
    "Edit.GetRandomBookmark.8",
    "Edit.GetRandomBookmark.9",
    "Edit.RemoveallBookmarks",
    "Edit.CopyBookmarkedLines",
    "Edit.CutBookmarkedLines",
    "Edit.DeleteBookmarkedLines",
    "Edit.TabifySelectionAll",
    "Edit.TabifySelectionLeading",
    "Edit.UntabifySelection",
    "Edit.TrimLeadingSpaces",
    "Edit.TrimExcessiveSpaces",
    "Edit.RemoveExcessiveBlankLines",
    "Edit.SplitLine",
    "Edit.NormalizeKeyword",
    "Edit.GoTo",
    "Edit.DatetimeStamp",
    "Script.ExecuteFromCursor",
    "Script.HaltOnError",
    "Script.NextError",
    "Script.PreviousError",
    "Script.ExecuteCurrentAndStep",
    "Script.StepForwardAndGetHistory",
    "Script.GetHistoryAndStepBack",
    "Script.SubstitutionVariables",
    "Script.ExecuteCurrentAlt",
    "Script.QuickQuery",
    "Script.LoadDDLWithDbmsMetadata",
    "Script.FindObject",
    "View.StatusBar",
    "View.Snippets",
    "View.CodeOutline",
    "View.OpenFiles",
    "View.FilePanel",
    "View.FindInFiles",
    "Help.AboutOpenEditor",
    "Help.SqlHelp",
    "View.History",
    "View.Properties",
    "Script.QuickCount",
    "Session.ToggleReadOnly",
    "File.Open",
    "File.Save",
    "File.SaveAs",
    "File.SaveAll",
    "File.Reload",
    "File.CopyFileLocation",
    "File.ShowFileLocation",
    "File.Exit",
    "File.Close",
    "File.CloseAll",
    "File.New",
    "File.NewExtended",
    "Session.Reconnect",
    "Session.RefreshObjectCache",
    "Tools.ExtractSchemaDDL",
    "Window.ActivateSplitter",
    "Window.SplitterDown",
    "Window.SplitterUp",
    "Session.Connect",
    "Session.Disconnect",
    "Session.Commit",
    "Session.Rollback",
    "Session.Cancel",
    "Session.EnableSessionStatistics",
    "Session.EnableDbmsOutput",
    "Script.Execute",
    "Script.ExecuteCurrent",
    "Script.ExplainPlan",
    "Script.LoadDDL",
    "View.ObjectList",
    "View.ObjectTree",
    "Workspace.Save",
    "Workspace.Open",
];

/// Commands the original `GUICommandDictionary` registers but
/// `menu_spec.json` does not list (toolbar entries and window commands).
const EXTRA: &[&str] = &[
    "Edit.Autocomplete",
    "Edit.FormatSQLwithSettings",
    "Edit.JumpNext",
    "Edit.JumpPrevious",
    "Edit.ToggleFoldingEnabled",
    "Grid.ShowPopupViewer",
    "Help.ContextHelp",
    "Help.Help",
    "Test",
    "Tools.SessionDdlGridSettings",
    "View.ExplainPlan",
    "View.NextPane",
    "View.ObjectTreeSetFocus",
    "View.PrevPane",
    "View.Toolbar",
    "Window.ArrangeIcons",
    "Window.Cascade",
    "Window.LastWindow",
    "Window.NewWindow",
    "Window.NextPane",
    "Window.NextTab",
    "Window.NextWindow",
    "Window.PrevWindow",
    "Window.PreviousPane",
    "Window.PreviousTab",
    "Window.TileHorizontally",
    "Window.TileVertically",
    "Workspace.OpenQuick",
];

pub fn registry() -> Vec<CommandSpec> {
    let root: Value = serde_json::from_str(include_str!("../../../../ui/spec/menu_spec.json"))
        .expect("menu_spec.json is valid");
    let mut out = Vec::new();
    if let Some(commands) = root.get("commands").and_then(Value::as_object) {
        for (id, value) in commands {
            let name = value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let keys = value
                .get("keys")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let help = value
                .get("help")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let implemented = !name.is_empty() && IMPLEMENTED.contains(&name.as_str());
            out.push(CommandSpec {
                id: id.clone(),
                name,
                keys,
                help,
                implemented,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    // The Windows dictionary has commands without a menu entry; expose them too
    // so the registry matches the original's command set.
    let known: std::collections::BTreeSet<String> =
        out.iter().map(|command| command.name.clone()).collect();
    for name in EXTRA {
        if known.contains(*name) {
            continue;
        }
        out.push(CommandSpec {
            id: format!("extra.{name}"),
            name: (*name).to_string(),
            keys: Vec::new(),
            help: String::new(),
            implemented: IMPLEMENTED.contains(name),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_original_commands() {
        let commands = registry();
        assert_eq!(commands.len(), 235 + EXTRA.len());
        assert!(commands
            .iter()
            .any(|command| command.name == "Script.Execute"));
        assert!(commands
            .iter()
            .any(|command| command.name == "Workspace.Save" && command.implemented));
        assert!(commands.iter().any(|command| command.name.is_empty()));
        for name in [
            "Edit.Find",
            "Edit.ToggleBookmark",
            "Edit.SelectLine",
            "Edit.TrimTrailingSpaces",
            "Edit.CollapseAllFolds",
            "Edit.ExpandCurrentSubtree",
            "Script.ExecuteInSQLPlus",
            "File.Print",
            "File.PrintPreview",
            "File.PrintSetup",
            "File.PageSetup",
            "Tools.TableTransformationHelper",
        ] {
            assert!(
                commands
                    .iter()
                    .any(|command| command.name == name && command.implemented),
                "{name} should be implemented"
            );
        }
        assert_eq!(
            commands
                .iter()
                .filter(|command| command.implemented)
                .count(),
            183
        );
    }
}
