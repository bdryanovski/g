
use builtin;
use str;

set edit:completion:arg-completer[g] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'g'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'g'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
            cand workspace 'Manage worktree-based workspaces (parallel branch checkouts)'
            cand stack 'Manage stacked pull requests'
            cand commit 'Interactive guided commit with message templates'
            cand add 'Stage files interactively, or forward arguments to `git add`'
            cand stage 'Interactive file-tree picker for staging and unstaging'
            cand compare 'Compare two branches visually'
            cand log 'Enhanced git log with beautiful formatting'
            cand status 'Enhanced git status with icons and colors'
            cand diff 'Enhanced git diff using your configured diff tool'
            cand branch 'Enhanced branch listing, `git branch` passthrough, or `branch squash`'
            cand show 'Enhanced git show'
            cand config 'Open interactive config editor'
            cand stats 'Display a rich usage-statistics report'
            cand developer 'Developer / debugging utilities'
            cand completions 'Print a shell completion script and exit'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'g;workspace'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand init 'Reorganise an existing repo into a container/worktree layout'
            cand list 'List all workspaces (git worktrees)'
            cand create 'Create a new workspace as a sibling worktree directory'
            cand switch 'Open a subshell in a workspace directory'
            cand delete 'Remove a workspace (git worktree remove)'
            cand status 'Show current workspace info'
            cand rename 'Rename a workspace (move directory and repair worktree)'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'g;workspace;init'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;workspace;list'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;workspace;create'= {
            cand -b 'Branch to check out (defaults to creating a new branch with the workspace name)'
            cand --branch 'Branch to check out (defaults to creating a new branch with the workspace name)'
            cand -d 'Description of this workspace'
            cand --description 'Description of this workspace'
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --copy 'Show an interactive picker to copy untracked/gitignored files into the new workspace'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;workspace;switch'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;workspace;delete'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --force 'Force removal even if the worktree is dirty'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;workspace;status'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;workspace;rename'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;workspace;help'= {
            cand init 'Reorganise an existing repo into a container/worktree layout'
            cand list 'List all workspaces (git worktrees)'
            cand create 'Create a new workspace as a sibling worktree directory'
            cand switch 'Open a subshell in a workspace directory'
            cand delete 'Remove a workspace (git worktree remove)'
            cand status 'Show current workspace info'
            cand rename 'Rename a workspace (move directory and repair worktree)'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'g;workspace;help;init'= {
        }
        &'g;workspace;help;list'= {
        }
        &'g;workspace;help;create'= {
        }
        &'g;workspace;help;switch'= {
        }
        &'g;workspace;help;delete'= {
        }
        &'g;workspace;help;status'= {
        }
        &'g;workspace;help;rename'= {
        }
        &'g;workspace;help;help'= {
        }
        &'g;stack'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand new 'Initialize a new stack starting from the current branch'
            cand add 'Create a new branch on top of the current stack'
            cand list 'List all stacks'
            cand view 'Show the current stack as a tree'
            cand details 'Show the current stack with commits for each branch'
            cand switch 'Switch to a different stack (checks out its top branch)'
            cand absorb 'Merge the current branch into the one below it in the stack'
            cand squash 'Squash the current branch to one commit on top of its base, then rebase branches above'
            cand fold 'Merge the current branch into its parent (preserving history), drop the extra ref, restack above'
            cand sync 'Sync all stack branches (rebase each on the one below)'
            cand push 'Push all branches in the current stack'
            cand pr 'Create or update GitHub PRs for all branches in the stack'
            cand remove 'Remove a branch from the stack (doesn''t delete the branch)'
            cand delete 'Delete a stack (and optionally its branches)'
            cand up 'Move a stack up or down in the stack list (affects display order and PR ordering)'
            cand down 'down'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'g;stack;new'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;add'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;list'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;view'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;details'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;switch'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;absorb'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;squash'= {
            cand -m 'Commit message for the squashed commit (default: oldest commit subject in the range)'
            cand --message 'Commit message for the squashed commit (default: oldest commit subject in the range)'
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --no-interactive 'Abort if any conflict is found instead of pausing'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;fold'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --keep 'Keep the current branch name as the combined branch (remove the parent ref from the stack)'
            cand --no-interactive 'Abort if merge/rebase hits conflicts instead of pausing for resolution'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;sync'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --no-interactive 'Abort if any conflict is found instead of pausing'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;push'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --force 'Force push with lease'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;pr'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --open 'Open PRs in browser after creating'
            cand --draft 'Draft PRs'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;remove'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;delete'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --branches 'Also delete all branches in the stack'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;up'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;down'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stack;help'= {
            cand new 'Initialize a new stack starting from the current branch'
            cand add 'Create a new branch on top of the current stack'
            cand list 'List all stacks'
            cand view 'Show the current stack as a tree'
            cand details 'Show the current stack with commits for each branch'
            cand switch 'Switch to a different stack (checks out its top branch)'
            cand absorb 'Merge the current branch into the one below it in the stack'
            cand squash 'Squash the current branch to one commit on top of its base, then rebase branches above'
            cand fold 'Merge the current branch into its parent (preserving history), drop the extra ref, restack above'
            cand sync 'Sync all stack branches (rebase each on the one below)'
            cand push 'Push all branches in the current stack'
            cand pr 'Create or update GitHub PRs for all branches in the stack'
            cand remove 'Remove a branch from the stack (doesn''t delete the branch)'
            cand delete 'Delete a stack (and optionally its branches)'
            cand up 'Move a stack up or down in the stack list (affects display order and PR ordering)'
            cand down 'down'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'g;stack;help;new'= {
        }
        &'g;stack;help;add'= {
        }
        &'g;stack;help;list'= {
        }
        &'g;stack;help;view'= {
        }
        &'g;stack;help;details'= {
        }
        &'g;stack;help;switch'= {
        }
        &'g;stack;help;absorb'= {
        }
        &'g;stack;help;squash'= {
        }
        &'g;stack;help;fold'= {
        }
        &'g;stack;help;sync'= {
        }
        &'g;stack;help;push'= {
        }
        &'g;stack;help;pr'= {
        }
        &'g;stack;help;remove'= {
        }
        &'g;stack;help;delete'= {
        }
        &'g;stack;help;up'= {
        }
        &'g;stack;help;down'= {
        }
        &'g;stack;help;help'= {
        }
        &'g;commit'= {
            cand -m 'Commit message subject (skips interactive mode)'
            cand --message 'Commit message subject (skips interactive mode)'
            cand -b 'Commit message body'
            cand --body 'Commit message body'
            cand --type 'Commit type (feat, fix, docs, etc.) — skips prompt'
            cand --scope 'Commit scope — skips prompt'
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --no-verify 'Don''t run pre-commit hooks'
            cand -a 'Stage all changes before committing'
            cand --all 'Stage all changes before committing'
            cand --amend 'Amend the last commit'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;add'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stage'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;compare'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --stat 'Only show file-level stat, not full diff'
            cand --diff 'Show full diff'
            cand --commits 'Show only commits'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;log'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;status'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;diff'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;branch'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand squash 'Collapse all commits on the current branch into one (from merge-base with base)'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'g;branch;squash'= {
            cand -m 'Commit message (default: oldest subject in the squashed range)'
            cand --message 'Commit message (default: oldest subject in the squashed range)'
            cand -b 'Ref to merge against when finding the fork point (`git merge-base HEAD <base>`)'
            cand --base 'Ref to merge against when finding the fork point (`git merge-base HEAD <base>`)'
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;branch;help'= {
            cand squash 'Collapse all commits on the current branch into one (from merge-base with base)'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'g;branch;help;squash'= {
        }
        &'g;branch;help;help'= {
        }
        &'g;show'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;config'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --edit 'Open config file in $EDITOR'
            cand --path 'Print the path to the config file'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;stats'= {
            cand --days 'Number of days to look back for time-based stats'
            cand --import-limit 'Maximum number of commits to import (default: all)'
            cand --search 'Search commit messages using fuzzy matching'
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --no-git 'Skip sections that require a git repository (heatmap, lines chart)'
            cand --import 'Import git commit history into the statistics database'
            cand --duplicates 'Show duplicate commit messages'
            cand --message-stats 'Show commit message length statistics and trends'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;developer'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand db 'Open an interactive SQLite shell connected to the internal g.db database'
            cand repos 'List all repositories tracked in the internal database'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'g;developer;db'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --path 'Print the database path and exit (don''t open the shell)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;developer;repos'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;developer;help'= {
            cand db 'Open an interactive SQLite shell connected to the internal g.db database'
            cand repos 'List all repositories tracked in the internal database'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'g;developer;help;db'= {
        }
        &'g;developer;help;repos'= {
        }
        &'g;developer;help;help'= {
        }
        &'g;completions'= {
            cand -C 'Run as if git was started in <path>'
            cand -c 'Override a configuration value (key=value)'
            cand --dry-run 'Preview what commands would run without making any changes'
            cand --no-interactive 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'g;help'= {
            cand workspace 'Manage worktree-based workspaces (parallel branch checkouts)'
            cand stack 'Manage stacked pull requests'
            cand commit 'Interactive guided commit with message templates'
            cand add 'Stage files interactively, or forward arguments to `git add`'
            cand stage 'Interactive file-tree picker for staging and unstaging'
            cand compare 'Compare two branches visually'
            cand log 'Enhanced git log with beautiful formatting'
            cand status 'Enhanced git status with icons and colors'
            cand diff 'Enhanced git diff using your configured diff tool'
            cand branch 'Enhanced branch listing, `git branch` passthrough, or `branch squash`'
            cand show 'Enhanced git show'
            cand config 'Open interactive config editor'
            cand stats 'Display a rich usage-statistics report'
            cand developer 'Developer / debugging utilities'
            cand completions 'Print a shell completion script and exit'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'g;help;workspace'= {
            cand init 'Reorganise an existing repo into a container/worktree layout'
            cand list 'List all workspaces (git worktrees)'
            cand create 'Create a new workspace as a sibling worktree directory'
            cand switch 'Open a subshell in a workspace directory'
            cand delete 'Remove a workspace (git worktree remove)'
            cand status 'Show current workspace info'
            cand rename 'Rename a workspace (move directory and repair worktree)'
        }
        &'g;help;workspace;init'= {
        }
        &'g;help;workspace;list'= {
        }
        &'g;help;workspace;create'= {
        }
        &'g;help;workspace;switch'= {
        }
        &'g;help;workspace;delete'= {
        }
        &'g;help;workspace;status'= {
        }
        &'g;help;workspace;rename'= {
        }
        &'g;help;stack'= {
            cand new 'Initialize a new stack starting from the current branch'
            cand add 'Create a new branch on top of the current stack'
            cand list 'List all stacks'
            cand view 'Show the current stack as a tree'
            cand details 'Show the current stack with commits for each branch'
            cand switch 'Switch to a different stack (checks out its top branch)'
            cand absorb 'Merge the current branch into the one below it in the stack'
            cand squash 'Squash the current branch to one commit on top of its base, then rebase branches above'
            cand fold 'Merge the current branch into its parent (preserving history), drop the extra ref, restack above'
            cand sync 'Sync all stack branches (rebase each on the one below)'
            cand push 'Push all branches in the current stack'
            cand pr 'Create or update GitHub PRs for all branches in the stack'
            cand remove 'Remove a branch from the stack (doesn''t delete the branch)'
            cand delete 'Delete a stack (and optionally its branches)'
            cand up 'Move a stack up or down in the stack list (affects display order and PR ordering)'
            cand down 'down'
        }
        &'g;help;stack;new'= {
        }
        &'g;help;stack;add'= {
        }
        &'g;help;stack;list'= {
        }
        &'g;help;stack;view'= {
        }
        &'g;help;stack;details'= {
        }
        &'g;help;stack;switch'= {
        }
        &'g;help;stack;absorb'= {
        }
        &'g;help;stack;squash'= {
        }
        &'g;help;stack;fold'= {
        }
        &'g;help;stack;sync'= {
        }
        &'g;help;stack;push'= {
        }
        &'g;help;stack;pr'= {
        }
        &'g;help;stack;remove'= {
        }
        &'g;help;stack;delete'= {
        }
        &'g;help;stack;up'= {
        }
        &'g;help;stack;down'= {
        }
        &'g;help;commit'= {
        }
        &'g;help;add'= {
        }
        &'g;help;stage'= {
        }
        &'g;help;compare'= {
        }
        &'g;help;log'= {
        }
        &'g;help;status'= {
        }
        &'g;help;diff'= {
        }
        &'g;help;branch'= {
            cand squash 'Collapse all commits on the current branch into one (from merge-base with base)'
        }
        &'g;help;branch;squash'= {
        }
        &'g;help;show'= {
        }
        &'g;help;config'= {
        }
        &'g;help;stats'= {
        }
        &'g;help;developer'= {
            cand db 'Open an interactive SQLite shell connected to the internal g.db database'
            cand repos 'List all repositories tracked in the internal database'
        }
        &'g;help;developer;db'= {
        }
        &'g;help;developer;repos'= {
        }
        &'g;help;completions'= {
        }
        &'g;help;help'= {
        }
    ]
    $completions[$command]
}
