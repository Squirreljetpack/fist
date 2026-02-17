# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_fs_global_optspecs
	string join \n verbosity= override= config= mm-config= dump-config style= help V/version
end

function __fish_fs_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_fs_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_fs_using_subcommand
	set -l cmd (__fish_fs_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c fs -n "__fish_fs_needs_command" -l verbosity -r
complete -c fs -n "__fish_fs_needs_command" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_needs_command" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_needs_command" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_needs_command" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_needs_command" -l dump-config -d 'Dump the main config and any other missing configuration files to default locations: If the output was detected to have been redirected, this prints the main configuration. Otherwise, this WILL OVERWRITE your main config.'
complete -c fs -n "__fish_fs_needs_command" -l help
complete -c fs -n "__fish_fs_needs_command" -s V -l version -d 'Print version'
complete -c fs -n "__fish_fs_needs_command" -f -a ":open" -d 'Launch apps and files'
complete -c fs -n "__fish_fs_needs_command" -f -a ":o" -d 'Launch apps and files'
complete -c fs -n "__fish_fs_needs_command" -f -a ":file" -d 'Recent files'
complete -c fs -n "__fish_fs_needs_command" -f -a ":dir" -d 'Recent folders'
complete -c fs -n "__fish_fs_needs_command" -f -a ":fd" -d 'Find and browse. (Default)'
complete -c fs -n "__fish_fs_needs_command" -f -a "::" -d 'Find and browse. (Default)'
complete -c fs -n "__fish_fs_needs_command" -f -a ":rg" -d 'Full text search'
complete -c fs -n "__fish_fs_needs_command" -f -a ":" -d 'Full text search'
complete -c fs -n "__fish_fs_needs_command" -f -a ":tool" -d 'Plugins and utilities'
complete -c fs -n "__fish_fs_needs_command" -f -a ":t" -d 'Plugins and utilities'
complete -c fs -n "__fish_fs_needs_command" -f -a ":info" -d 'Stats and database records'
complete -c fs -n "__fish_fs_using_subcommand :open" -s w -l with -d 'app to open files with' -r
complete -c fs -n "__fish_fs_using_subcommand :open" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :open" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :open" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :open" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :open" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :open" -l list
complete -c fs -n "__fish_fs_using_subcommand :open" -l help -d 'initial query'
complete -c fs -n "__fish_fs_using_subcommand :o" -s w -l with -d 'app to open files with' -r
complete -c fs -n "__fish_fs_using_subcommand :o" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :o" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :o" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :o" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :o" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :o" -l list
complete -c fs -n "__fish_fs_using_subcommand :o" -l help -d 'initial query'
complete -c fs -n "__fish_fs_using_subcommand :file" -l sort -d 'history sort order' -r -f -a "name\t''
atime\t''
frecency\t'Weighted frequency + recency'
count\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :file" -s l -l list -r -f -a "_\t''
all\t''"
complete -c fs -n "__fish_fs_using_subcommand :file" -l query -d 'initial query' -r
complete -c fs -n "__fish_fs_using_subcommand :file" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :file" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :file" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :file" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :file" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :file" -l help
complete -c fs -n "__fish_fs_using_subcommand :dir" -l sort -d 'history sort order' -r -f -a "name\t''
atime\t''
frecency\t'Weighted frequency + recency'
count\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :dir" -s l -l list -r -f -a "_\t''
all\t''"
complete -c fs -n "__fish_fs_using_subcommand :dir" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :dir" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :dir" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :dir" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :dir" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :dir" -l cd -d 'print the first match'
complete -c fs -n "__fish_fs_using_subcommand :dir" -l help
complete -c fs -n "__fish_fs_using_subcommand :fd" -l sort -r -f -a "name\t''
mtime\t''
none\t''
size\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -s t -l types -d 'restrict search to certain file types and extensions (use `:t types` to list)' -r
complete -c fs -n "__fish_fs_using_subcommand :fd" -l list-fmt -d 'Template to format the list output as' -r
complete -c fs -n "__fish_fs_using_subcommand :fd" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :fd" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :fd" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :fd" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :fd" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -s h -d 'show hidden files and folders'
complete -c fs -n "__fish_fs_using_subcommand :fd" -s I -d 'HIDE ignored files'
complete -c fs -n "__fish_fs_using_subcommand :fd" -s a -d 'show all'
complete -c fs -n "__fish_fs_using_subcommand :fd" -s F -d 'only show directories'
complete -c fs -n "__fish_fs_using_subcommand :fd" -s f -d 'show only files'
complete -c fs -n "__fish_fs_using_subcommand :fd" -l list
complete -c fs -n "__fish_fs_using_subcommand :fd" -l no-read -d 'Never stream input from stdin'
complete -c fs -n "__fish_fs_using_subcommand :fd" -l cd -d 'print the first match'
complete -c fs -n "__fish_fs_using_subcommand :fd" -l help
complete -c fs -n "__fish_fs_using_subcommand ::" -l sort -r -f -a "name\t''
mtime\t''
none\t''
size\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -s t -l types -d 'restrict search to certain file types and extensions (use `:t types` to list)' -r
complete -c fs -n "__fish_fs_using_subcommand ::" -l list-fmt -d 'Template to format the list output as' -r
complete -c fs -n "__fish_fs_using_subcommand ::" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand ::" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand ::" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand ::" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand ::" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -s h -d 'show hidden files and folders'
complete -c fs -n "__fish_fs_using_subcommand ::" -s I -d 'HIDE ignored files'
complete -c fs -n "__fish_fs_using_subcommand ::" -s a -d 'show all'
complete -c fs -n "__fish_fs_using_subcommand ::" -s F -d 'only show directories'
complete -c fs -n "__fish_fs_using_subcommand ::" -s f -d 'show only files'
complete -c fs -n "__fish_fs_using_subcommand ::" -l list
complete -c fs -n "__fish_fs_using_subcommand ::" -l no-read -d 'Never stream input from stdin'
complete -c fs -n "__fish_fs_using_subcommand ::" -l cd -d 'print the first match'
complete -c fs -n "__fish_fs_using_subcommand ::" -l help
complete -c fs -n "__fish_fs_using_subcommand :rg" -l query -d 'initial query' -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :rg" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :rg" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -s h -d 'show hidden files and folders'
complete -c fs -n "__fish_fs_using_subcommand :rg" -s I -d 'HIDE ignored files'
complete -c fs -n "__fish_fs_using_subcommand :rg" -s a -d 'show all'
complete -c fs -n "__fish_fs_using_subcommand :rg" -s F -d 'only show directories'
complete -c fs -n "__fish_fs_using_subcommand :rg" -s f -d 'show only files'
complete -c fs -n "__fish_fs_using_subcommand :rg" -l help
complete -c fs -n "__fish_fs_using_subcommand :" -l query -d 'initial query' -r
complete -c fs -n "__fish_fs_using_subcommand :" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -s h -d 'show hidden files and folders'
complete -c fs -n "__fish_fs_using_subcommand :" -s I -d 'HIDE ignored files'
complete -c fs -n "__fish_fs_using_subcommand :" -s a -d 'show all'
complete -c fs -n "__fish_fs_using_subcommand :" -s F -d 'only show directories'
complete -c fs -n "__fish_fs_using_subcommand :" -s f -d 'show only files'
complete -c fs -n "__fish_fs_using_subcommand :" -l help
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l help
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "colors"
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "liza" -d 'List directory (eza wrapper)'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "shell" -d 'Dump the initialization code for your shell'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "lessfilter" -d 'Context and preset dependent file handler'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "bump" -d 'Bump history entries'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "types" -d 'List mappings supported by the --type parameter'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l z-name -d 'Name for jump function' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l z-dot-args -d 'Arguments passed to `fs ::` when z is invoked with a trailing `.`' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l z-slash-args -d 'Arguments passed to `fs ::` when z is invoked with a trailing `./`' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l z-sort -d 'Default sort order for the interactive jump menu' -r -f -a "name\t''
atime\t''
frecency\t'Weighted frequency + recency'
count\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l open-name -d 'Name for open function' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l open-cmd -d 'Command used by open function' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l dir-widget-bind -d 'Bind for the directory widget' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l file-widget-bind -d 'Bind for the directory widget' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l rg-widget-bind -d 'Bind for the directory widget' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l file-open-cmd -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l rg-open-cmd -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l dir-widget-args -d 'Arguments passed to `fs ::` when dir widget is invoked' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l file-widget-args -d 'Arguments passed to `fs ::` when file widget is invoked' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l rg-widget-args -d 'Arguments passed to `fs :` when rg widget is invoked' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l shell -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l aliases
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l header -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -s g -l glob -d 'glob pattern to bump' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -s c -l count -d 'amount to bump by, 0 to clear' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l reset -d 'reset the database'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -l help
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "colors"
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "liza" -d 'List directory (eza wrapper)'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "shell" -d 'Dump the initialization code for your shell'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "lessfilter" -d 'Context and preset dependent file handler'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "bump" -d 'Bump history entries'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter bump types" -a "types" -d 'List mappings supported by the --type parameter'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l z-name -d 'Name for jump function' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l z-dot-args -d 'Arguments passed to `fs ::` when z is invoked with a trailing `.`' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l z-slash-args -d 'Arguments passed to `fs ::` when z is invoked with a trailing `./`' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l z-sort -d 'Default sort order for the interactive jump menu' -r -f -a "name\t''
atime\t''
frecency\t'Weighted frequency + recency'
count\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l open-name -d 'Name for open function' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l open-cmd -d 'Command used by open function' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l dir-widget-bind -d 'Bind for the directory widget' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l file-widget-bind -d 'Bind for the directory widget' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l rg-widget-bind -d 'Bind for the directory widget' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l file-open-cmd -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l rg-open-cmd -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l dir-widget-args -d 'Arguments passed to `fs ::` when dir widget is invoked' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l file-widget-args -d 'Arguments passed to `fs ::` when file widget is invoked' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l rg-widget-args -d 'Arguments passed to `fs :` when rg widget is invoked' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l shell -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l aliases
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l header -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -s g -l glob -d 'glob pattern to bump' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -s c -l count -d 'amount to bump by, 0 to clear' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l reset -d 'reset the database'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :info" -l sort -d 'history sort order' -r -f -a "name\t''
atime\t''
frecency\t'Weighted frequency + recency'
count\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :info" -s l -l limit -d 'maximum history entries to display' -r
complete -c fs -n "__fish_fs_using_subcommand :info" -l verbosity -r
complete -c fs -n "__fish_fs_using_subcommand :info" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :info" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :info" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :info" -l style -r -f -a "icons\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :info" -s m -l minimal -d 'Don\'t print decorations'
complete -c fs -n "__fish_fs_using_subcommand :info" -l help
