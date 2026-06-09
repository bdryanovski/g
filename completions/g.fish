# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_g_global_optspecs
	string join \n C= c= dry-run no-interactive h/help V/version
end

function __fish_g_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_g_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_g_using_subcommand
	set -l cmd (__fish_g_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c g -n "__fish_g_needs_command" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_needs_command" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_needs_command" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_needs_command" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c g -n "__fish_g_needs_command" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_needs_command" -f -a "workspace" -d 'Manage worktree-based workspaces (parallel branch checkouts)'
complete -c g -n "__fish_g_needs_command" -f -a "stack" -d 'Manage stacked pull requests'
complete -c g -n "__fish_g_needs_command" -f -a "commit" -d 'Interactive guided commit with message templates'
complete -c g -n "__fish_g_needs_command" -f -a "add" -d 'Stage files interactively, or forward arguments to `git add`'
complete -c g -n "__fish_g_needs_command" -f -a "stage" -d 'Interactive file-tree picker for staging and unstaging'
complete -c g -n "__fish_g_needs_command" -f -a "compare" -d 'Compare two branches visually'
complete -c g -n "__fish_g_needs_command" -f -a "log" -d 'Enhanced git log with beautiful formatting'
complete -c g -n "__fish_g_needs_command" -f -a "status" -d 'Enhanced git status with icons and colors'
complete -c g -n "__fish_g_needs_command" -f -a "diff" -d 'Enhanced git diff using your configured diff tool'
complete -c g -n "__fish_g_needs_command" -f -a "branch" -d 'Enhanced branch listing, `git branch` passthrough, or `branch squash`'
complete -c g -n "__fish_g_needs_command" -f -a "show" -d 'Enhanced git show'
complete -c g -n "__fish_g_needs_command" -f -a "config" -d 'Open interactive config editor'
complete -c g -n "__fish_g_needs_command" -f -a "stats" -d 'Display a rich usage-statistics report'
complete -c g -n "__fish_g_needs_command" -f -a "developer" -d 'Developer / debugging utilities'
complete -c g -n "__fish_g_needs_command" -f -a "completions" -d 'Print a shell completion script and exit'
complete -c g -n "__fish_g_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -f -a "init" -d 'Reorganise an existing repo into a container/worktree layout'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -f -a "list" -d 'List all workspaces (git worktrees)'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -f -a "create" -d 'Create a new workspace as a sibling worktree directory'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -f -a "switch" -d 'Open a subshell in a workspace directory'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -f -a "delete" -d 'Remove a workspace (git worktree remove)'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -f -a "status" -d 'Show current workspace info'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -f -a "rename" -d 'Rename a workspace (move directory and repair worktree)'
complete -c g -n "__fish_g_using_subcommand workspace; and not __fish_seen_subcommand_from init list create switch delete status rename help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from init" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from init" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from init" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from init" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from init" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from init" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from list" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from list" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from list" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from list" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from list" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from create" -s b -l branch -d 'Branch to check out (defaults to creating a new branch with the workspace name)' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from create" -s d -l description -d 'Description of this workspace' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from create" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from create" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from create" -l copy -d 'Show an interactive picker to copy untracked/gitignored files into the new workspace'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from create" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from create" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from create" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from create" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from switch" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from switch" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from switch" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from switch" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from switch" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from switch" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from delete" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from delete" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from delete" -l force -d 'Force removal even if the worktree is dirty'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from delete" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from delete" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from delete" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from delete" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from status" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from status" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from status" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from status" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from status" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from status" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from rename" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from rename" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from rename" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from rename" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from rename" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from rename" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from help" -f -a "init" -d 'Reorganise an existing repo into a container/worktree layout'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from help" -f -a "list" -d 'List all workspaces (git worktrees)'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from help" -f -a "create" -d 'Create a new workspace as a sibling worktree directory'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from help" -f -a "switch" -d 'Open a subshell in a workspace directory'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from help" -f -a "delete" -d 'Remove a workspace (git worktree remove)'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from help" -f -a "status" -d 'Show current workspace info'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from help" -f -a "rename" -d 'Rename a workspace (move directory and repair worktree)'
complete -c g -n "__fish_g_using_subcommand workspace; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "new" -d 'Initialize a new stack starting from the current branch'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "add" -d 'Create a new branch on top of the current stack'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "list" -d 'List all stacks'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "view" -d 'Show the current stack as a tree'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "details" -d 'Show the current stack with commits for each branch'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "switch" -d 'Switch to a different stack (checks out its top branch)'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "absorb" -d 'Merge the current branch into the one below it in the stack'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "squash" -d 'Squash the current branch to one commit on top of its base, then rebase branches above'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "fold" -d 'Merge the current branch into its parent (preserving history), drop the extra ref, restack above'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "sync" -d 'Sync all stack branches (rebase each on the one below)'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "push" -d 'Push all branches in the current stack'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "pr" -d 'Create or update GitHub PRs for all branches in the stack'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "remove" -d 'Remove a branch from the stack (doesn\'t delete the branch)'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "delete" -d 'Delete a stack (and optionally its branches)'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "up" -d 'Move a stack up or down in the stack list (affects display order and PR ordering)'
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "down"
complete -c g -n "__fish_g_using_subcommand stack; and not __fish_seen_subcommand_from new add list view details switch absorb squash fold sync push pr remove delete up down help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from new" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from new" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from new" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from new" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from new" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from new" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from add" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from add" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from add" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from add" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from add" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from add" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from list" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from list" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from list" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from list" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from list" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from view" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from view" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from view" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from view" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from view" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from view" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from details" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from details" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from details" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from details" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from details" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from details" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from switch" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from switch" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from switch" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from switch" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from switch" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from switch" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from absorb" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from absorb" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from absorb" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from absorb" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from absorb" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from absorb" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from squash" -s m -l message -d 'Commit message for the squashed commit (default: oldest commit subject in the range)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from squash" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from squash" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from squash" -l no-interactive -d 'Abort if any conflict is found instead of pausing'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from squash" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from squash" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from squash" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from fold" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from fold" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from fold" -l keep -d 'Keep the current branch name as the combined branch (remove the parent ref from the stack)'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from fold" -l no-interactive -d 'Abort if merge/rebase hits conflicts instead of pausing for resolution'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from fold" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from fold" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from fold" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from sync" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from sync" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from sync" -l no-interactive -d 'Abort if any conflict is found instead of pausing'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from sync" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from sync" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from sync" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from push" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from push" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from push" -l force -d 'Force push with lease'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from push" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from push" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from push" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from push" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from pr" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from pr" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from pr" -l open -d 'Open PRs in browser after creating'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from pr" -l draft -d 'Draft PRs'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from pr" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from pr" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from pr" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from pr" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from remove" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from remove" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from remove" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from remove" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from remove" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from remove" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from delete" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from delete" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from delete" -l branches -d 'Also delete all branches in the stack'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from delete" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from delete" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from delete" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from delete" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from up" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from up" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from up" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from up" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from up" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from up" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from down" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from down" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from down" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from down" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from down" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from down" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "new" -d 'Initialize a new stack starting from the current branch'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "add" -d 'Create a new branch on top of the current stack'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "list" -d 'List all stacks'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "view" -d 'Show the current stack as a tree'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "details" -d 'Show the current stack with commits for each branch'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "switch" -d 'Switch to a different stack (checks out its top branch)'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "absorb" -d 'Merge the current branch into the one below it in the stack'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "squash" -d 'Squash the current branch to one commit on top of its base, then rebase branches above'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "fold" -d 'Merge the current branch into its parent (preserving history), drop the extra ref, restack above'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "sync" -d 'Sync all stack branches (rebase each on the one below)'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "push" -d 'Push all branches in the current stack'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "pr" -d 'Create or update GitHub PRs for all branches in the stack'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "remove" -d 'Remove a branch from the stack (doesn\'t delete the branch)'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "delete" -d 'Delete a stack (and optionally its branches)'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "up" -d 'Move a stack up or down in the stack list (affects display order and PR ordering)'
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "down"
complete -c g -n "__fish_g_using_subcommand stack; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c g -n "__fish_g_using_subcommand commit" -s m -l message -d 'Commit message subject (skips interactive mode)' -r
complete -c g -n "__fish_g_using_subcommand commit" -s b -l body -d 'Commit message body' -r
complete -c g -n "__fish_g_using_subcommand commit" -l type -d 'Commit type (feat, fix, docs, etc.) — skips prompt' -r
complete -c g -n "__fish_g_using_subcommand commit" -l scope -d 'Commit scope — skips prompt' -r
complete -c g -n "__fish_g_using_subcommand commit" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand commit" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand commit" -l no-verify -d 'Don\'t run pre-commit hooks'
complete -c g -n "__fish_g_using_subcommand commit" -s a -l all -d 'Stage all changes before committing'
complete -c g -n "__fish_g_using_subcommand commit" -l amend -d 'Amend the last commit'
complete -c g -n "__fish_g_using_subcommand commit" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand commit" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand commit" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand commit" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand add" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand add" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand add" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand add" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand add" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c g -n "__fish_g_using_subcommand add" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stage" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stage" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stage" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stage" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stage" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c g -n "__fish_g_using_subcommand stage" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand compare" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand compare" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand compare" -l stat -d 'Only show file-level stat, not full diff'
complete -c g -n "__fish_g_using_subcommand compare" -l diff -d 'Show full diff'
complete -c g -n "__fish_g_using_subcommand compare" -l commits -d 'Show only commits'
complete -c g -n "__fish_g_using_subcommand compare" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand compare" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand compare" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand compare" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand log" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand log" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand log" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand log" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand log" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand log" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand status" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand status" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand status" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand status" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand status" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand status" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand diff" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand diff" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand diff" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand diff" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand diff" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand diff" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand branch; and not __fish_seen_subcommand_from squash help" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand branch; and not __fish_seen_subcommand_from squash help" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand branch; and not __fish_seen_subcommand_from squash help" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand branch; and not __fish_seen_subcommand_from squash help" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand branch; and not __fish_seen_subcommand_from squash help" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand branch; and not __fish_seen_subcommand_from squash help" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand branch; and not __fish_seen_subcommand_from squash help" -a "squash" -d 'Collapse all commits on the current branch into one (from merge-base with base)'
complete -c g -n "__fish_g_using_subcommand branch; and not __fish_seen_subcommand_from squash help" -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c g -n "__fish_g_using_subcommand branch; and __fish_seen_subcommand_from squash" -s m -l message -d 'Commit message (default: oldest subject in the squashed range)' -r
complete -c g -n "__fish_g_using_subcommand branch; and __fish_seen_subcommand_from squash" -s b -l base -d 'Ref to merge against when finding the fork point (`git merge-base HEAD <base>`)' -r
complete -c g -n "__fish_g_using_subcommand branch; and __fish_seen_subcommand_from squash" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand branch; and __fish_seen_subcommand_from squash" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand branch; and __fish_seen_subcommand_from squash" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand branch; and __fish_seen_subcommand_from squash" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand branch; and __fish_seen_subcommand_from squash" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand branch; and __fish_seen_subcommand_from squash" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand branch; and __fish_seen_subcommand_from help" -f -a "squash" -d 'Collapse all commits on the current branch into one (from merge-base with base)'
complete -c g -n "__fish_g_using_subcommand branch; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c g -n "__fish_g_using_subcommand show" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand show" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand show" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand show" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand show" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand show" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand config" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand config" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand config" -l edit -d 'Open config file in $EDITOR'
complete -c g -n "__fish_g_using_subcommand config" -l path -d 'Print the path to the config file'
complete -c g -n "__fish_g_using_subcommand config" -l themes -d 'List available themes (built-in + custom) and exit'
complete -c g -n "__fish_g_using_subcommand config" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand config" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand config" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand config" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand stats" -l days -d 'Number of days to look back for time-based stats' -r
complete -c g -n "__fish_g_using_subcommand stats" -l import-limit -d 'Maximum number of commits to import (default: all)' -r
complete -c g -n "__fish_g_using_subcommand stats" -l search -d 'Search commit messages using fuzzy matching' -r
complete -c g -n "__fish_g_using_subcommand stats" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand stats" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand stats" -l no-git -d 'Skip sections that require a git repository (heatmap, lines chart)'
complete -c g -n "__fish_g_using_subcommand stats" -l import -d 'Import git commit history into the statistics database'
complete -c g -n "__fish_g_using_subcommand stats" -l duplicates -d 'Show duplicate commit messages'
complete -c g -n "__fish_g_using_subcommand stats" -l message-stats -d 'Show commit message length statistics and trends'
complete -c g -n "__fish_g_using_subcommand stats" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand stats" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand stats" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c g -n "__fish_g_using_subcommand stats" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand developer; and not __fish_seen_subcommand_from db repos help" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand developer; and not __fish_seen_subcommand_from db repos help" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand developer; and not __fish_seen_subcommand_from db repos help" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand developer; and not __fish_seen_subcommand_from db repos help" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand developer; and not __fish_seen_subcommand_from db repos help" -s h -l help -d 'Print help'
complete -c g -n "__fish_g_using_subcommand developer; and not __fish_seen_subcommand_from db repos help" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand developer; and not __fish_seen_subcommand_from db repos help" -f -a "db" -d 'Open an interactive SQLite shell connected to the internal g.db database'
complete -c g -n "__fish_g_using_subcommand developer; and not __fish_seen_subcommand_from db repos help" -f -a "repos" -d 'List all repositories tracked in the internal database'
complete -c g -n "__fish_g_using_subcommand developer; and not __fish_seen_subcommand_from db repos help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from db" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from db" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from db" -l path -d 'Print the database path and exit (don\'t open the shell)'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from db" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from db" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from db" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from db" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from repos" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from repos" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from repos" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from repos" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from repos" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from repos" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from help" -f -a "db" -d 'Open an interactive SQLite shell connected to the internal g.db database'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from help" -f -a "repos" -d 'List all repositories tracked in the internal database'
complete -c g -n "__fish_g_using_subcommand developer; and __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c g -n "__fish_g_using_subcommand completions" -s C -d 'Run as if git was started in <path>' -r
complete -c g -n "__fish_g_using_subcommand completions" -s c -d 'Override a configuration value (key=value)' -r
complete -c g -n "__fish_g_using_subcommand completions" -l dry-run -d 'Preview what commands would run without making any changes'
complete -c g -n "__fish_g_using_subcommand completions" -l no-interactive -d 'Disable all interactive TUI prompts; use defaults or require --flag values. Useful for scripting and CI environments'
complete -c g -n "__fish_g_using_subcommand completions" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c g -n "__fish_g_using_subcommand completions" -s V -l version -d 'Print version'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "workspace" -d 'Manage worktree-based workspaces (parallel branch checkouts)'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "stack" -d 'Manage stacked pull requests'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "commit" -d 'Interactive guided commit with message templates'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "add" -d 'Stage files interactively, or forward arguments to `git add`'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "stage" -d 'Interactive file-tree picker for staging and unstaging'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "compare" -d 'Compare two branches visually'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "log" -d 'Enhanced git log with beautiful formatting'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "status" -d 'Enhanced git status with icons and colors'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "diff" -d 'Enhanced git diff using your configured diff tool'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "branch" -d 'Enhanced branch listing, `git branch` passthrough, or `branch squash`'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "show" -d 'Enhanced git show'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "config" -d 'Open interactive config editor'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "stats" -d 'Display a rich usage-statistics report'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "developer" -d 'Developer / debugging utilities'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "completions" -d 'Print a shell completion script and exit'
complete -c g -n "__fish_g_using_subcommand help; and not __fish_seen_subcommand_from workspace stack commit add stage compare log status diff branch show config stats developer completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from workspace" -f -a "init" -d 'Reorganise an existing repo into a container/worktree layout'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from workspace" -f -a "list" -d 'List all workspaces (git worktrees)'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from workspace" -f -a "create" -d 'Create a new workspace as a sibling worktree directory'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from workspace" -f -a "switch" -d 'Open a subshell in a workspace directory'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from workspace" -f -a "delete" -d 'Remove a workspace (git worktree remove)'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from workspace" -f -a "status" -d 'Show current workspace info'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from workspace" -f -a "rename" -d 'Rename a workspace (move directory and repair worktree)'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "new" -d 'Initialize a new stack starting from the current branch'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "add" -d 'Create a new branch on top of the current stack'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "list" -d 'List all stacks'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "view" -d 'Show the current stack as a tree'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "details" -d 'Show the current stack with commits for each branch'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "switch" -d 'Switch to a different stack (checks out its top branch)'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "absorb" -d 'Merge the current branch into the one below it in the stack'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "squash" -d 'Squash the current branch to one commit on top of its base, then rebase branches above'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "fold" -d 'Merge the current branch into its parent (preserving history), drop the extra ref, restack above'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "sync" -d 'Sync all stack branches (rebase each on the one below)'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "push" -d 'Push all branches in the current stack'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "pr" -d 'Create or update GitHub PRs for all branches in the stack'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "remove" -d 'Remove a branch from the stack (doesn\'t delete the branch)'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "delete" -d 'Delete a stack (and optionally its branches)'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "up" -d 'Move a stack up or down in the stack list (affects display order and PR ordering)'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from stack" -f -a "down"
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from branch" -f -a "squash" -d 'Collapse all commits on the current branch into one (from merge-base with base)'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from developer" -f -a "db" -d 'Open an interactive SQLite shell connected to the internal g.db database'
complete -c g -n "__fish_g_using_subcommand help; and __fish_seen_subcommand_from developer" -f -a "repos" -d 'List all repositories tracked in the internal database'
