# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_fs_global_optspecs
    string join \n q v override= config= mm-config= dump-config style= fullscreen= lock-prompt= alt-accept output-sep= format= opener= sort= h= I= a= F= f= A/no-all cd t/types= transform= reset-visibility list help V/version
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

complete -c fs -n "__fish_fs_needs_command" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_needs_command" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_needs_command" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_needs_command" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_needs_command" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_needs_command" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_needs_command" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_needs_command" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_needs_command" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_needs_command" -l sort -r -f -a "name\t''
mtime\t''
atime\t''
size\t''
none\t''"
complete -c fs -n "__fish_fs_needs_command" -s h -d 'show hidden files and folders' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_needs_command" -s I -d 'HIDE ignored files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_needs_command" -s a -d 'show all' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_needs_command" -s F -d 'only show directories' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_needs_command" -s f -d 'show only files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_needs_command" -s t -l types -d 'restrict search to certain file types and extensions (`:t types` to list)' -r
complete -c fs -n "__fish_fs_needs_command" -l transform -d 'Lua transform (path, tail) -> (path, display, tail). Missing display/tail keep the current values; a missing path omits the entry. Accepts a file when prefixed with @' -r
complete -c fs -n "__fish_fs_needs_command" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_needs_command" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_needs_command" -l dump-config -d 'Dump the main config and any other missing configuration files to default locations: If the output was detected to have been redirected, this prints the main configuration. Otherwise, this will OVERWRITE your main config.'
complete -c fs -n "__fish_fs_needs_command" -l alt-accept
complete -c fs -n "__fish_fs_needs_command" -s A -l no-all
complete -c fs -n "__fish_fs_needs_command" -l cd -d 'print the first match'
complete -c fs -n "__fish_fs_needs_command" -l reset-visibility
complete -c fs -n "__fish_fs_needs_command" -l list
complete -c fs -n "__fish_fs_needs_command" -l help
complete -c fs -n "__fish_fs_needs_command" -s V -l version -d 'Print version'
complete -c fs -n "__fish_fs_needs_command" -a ":open" -d 'Launch apps and files'
complete -c fs -n "__fish_fs_needs_command" -a ":o" -d 'Launch apps and files'
complete -c fs -n "__fish_fs_needs_command" -a ":file" -d 'Recent files'
complete -c fs -n "__fish_fs_needs_command" -a ":dir" -d 'Recent folders'
complete -c fs -n "__fish_fs_needs_command" -a ":fd" -d 'Find and browse. (Default)'
complete -c fs -n "__fish_fs_needs_command" -a "::" -d 'Find and browse. (Default)'
complete -c fs -n "__fish_fs_needs_command" -a ":custom" -d 'Browse a piped listing or a command\'s output'
complete -c fs -n "__fish_fs_needs_command" -a ":c" -d 'Browse a piped listing or a command\'s output'
complete -c fs -n "__fish_fs_needs_command" -a ":rg" -d 'Full text search'
complete -c fs -n "__fish_fs_needs_command" -a ":" -d 'Full text search'
complete -c fs -n "__fish_fs_needs_command" -a ":tool" -d 'Plugins and utilities'
complete -c fs -n "__fish_fs_needs_command" -a ":t" -d 'Plugins and utilities'
complete -c fs -n "__fish_fs_needs_command" -a ":info" -d 'Stats and database records'
complete -c fs -n "__fish_fs_using_subcommand :open" -s w -l with -d 'app to open files with' -r
complete -c fs -n "__fish_fs_using_subcommand :open" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :open" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :open" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :open" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :open" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :open" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :open" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :open" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :open" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :open" -l list
complete -c fs -n "__fish_fs_using_subcommand :open" -l help -d 'initial query'
complete -c fs -n "__fish_fs_using_subcommand :open" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :open" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :open" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :o" -s w -l with -d 'app to open files with' -r
complete -c fs -n "__fish_fs_using_subcommand :o" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :o" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :o" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :o" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :o" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :o" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :o" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :o" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :o" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :o" -l list
complete -c fs -n "__fish_fs_using_subcommand :o" -l help -d 'initial query'
complete -c fs -n "__fish_fs_using_subcommand :o" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :o" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :o" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :file" -l sort -d 'history sort order' -r -f -a "name\t''
mtime\t''
atime\t''
size\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :file" -s l -l list -r -f -a "_\t''
all\t''"
complete -c fs -n "__fish_fs_using_subcommand :file" -l query -d 'initial query' -r
complete -c fs -n "__fish_fs_using_subcommand :file" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :file" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :file" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :file" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :file" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :file" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :file" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :file" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :file" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :file" -l help
complete -c fs -n "__fish_fs_using_subcommand :file" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :file" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :file" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :dir" -l sort -d 'history sort order' -r -f -a "name\t''
mtime\t''
atime\t''
size\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :dir" -s l -l list -r -f -a "_\t''
all\t''"
complete -c fs -n "__fish_fs_using_subcommand :dir" -l initial-input -r
complete -c fs -n "__fish_fs_using_subcommand :dir" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :dir" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :dir" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :dir" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :dir" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :dir" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :dir" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :dir" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :dir" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :dir" -l cd -d 'print the first match'
complete -c fs -n "__fish_fs_using_subcommand :dir" -l help
complete -c fs -n "__fish_fs_using_subcommand :dir" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :dir" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :dir" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :fd" -l sort -r -f -a "name\t''
mtime\t''
atime\t''
size\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -s h -d 'show hidden files and folders' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -s I -d 'HIDE ignored files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -s a -d 'show all' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -s F -d 'only show directories' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -s f -d 'show only files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -s t -l types -d 'restrict search to certain file types and extensions (`:t types` to list)' -r
complete -c fs -n "__fish_fs_using_subcommand :fd" -l transform -d 'Lua transform (path, tail) -> (path, display, tail). Missing display/tail keep the current values; a missing path omits the entry. Accepts a file when prefixed with @' -r
complete -c fs -n "__fish_fs_using_subcommand :fd" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :fd" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :fd" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :fd" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :fd" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :fd" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :fd" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :fd" -s A -l no-all
complete -c fs -n "__fish_fs_using_subcommand :fd" -l cd -d 'print the first match'
complete -c fs -n "__fish_fs_using_subcommand :fd" -l reset-visibility
complete -c fs -n "__fish_fs_using_subcommand :fd" -l list
complete -c fs -n "__fish_fs_using_subcommand :fd" -l help
complete -c fs -n "__fish_fs_using_subcommand :fd" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :fd" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :fd" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand ::" -l sort -r -f -a "name\t''
mtime\t''
atime\t''
size\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -s h -d 'show hidden files and folders' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -s I -d 'HIDE ignored files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -s a -d 'show all' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -s F -d 'only show directories' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -s f -d 'show only files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -s t -l types -d 'restrict search to certain file types and extensions (`:t types` to list)' -r
complete -c fs -n "__fish_fs_using_subcommand ::" -l transform -d 'Lua transform (path, tail) -> (path, display, tail). Missing display/tail keep the current values; a missing path omits the entry. Accepts a file when prefixed with @' -r
complete -c fs -n "__fish_fs_using_subcommand ::" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand ::" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand ::" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand ::" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand ::" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand ::" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand ::" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand ::" -s A -l no-all
complete -c fs -n "__fish_fs_using_subcommand ::" -l cd -d 'print the first match'
complete -c fs -n "__fish_fs_using_subcommand ::" -l reset-visibility
complete -c fs -n "__fish_fs_using_subcommand ::" -l list
complete -c fs -n "__fish_fs_using_subcommand ::" -l help
complete -c fs -n "__fish_fs_using_subcommand ::" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand ::" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand ::" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :custom" -s h -d 'show hidden files and folders' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :custom" -s I -d 'HIDE ignored files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :custom" -s a -d 'show all' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :custom" -s F -d 'only show directories' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :custom" -s f -d 'show only files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :custom" -l sort -r -f -a "name\t''
mtime\t''
atime\t''
size\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :custom" -l transform -d 'Lua transform (path, tail) -> (path, display, tail)' -r
complete -c fs -n "__fish_fs_using_subcommand :custom" -l tail-sep -d 'Delimiter used to split off the input into a (path, tail) pair' -r
complete -c fs -n "__fish_fs_using_subcommand :custom" -l input-sep -d 'Split the stream on this character instead of newlines' -r
complete -c fs -n "__fish_fs_using_subcommand :custom" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :custom" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :custom" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :custom" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :custom" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :custom" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :custom" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :custom" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :custom" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :custom" -l cd -d 'print the first match'
complete -c fs -n "__fish_fs_using_subcommand :custom" -l help
complete -c fs -n "__fish_fs_using_subcommand :custom" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :custom" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :custom" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :c" -s h -d 'show hidden files and folders' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :c" -s I -d 'HIDE ignored files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :c" -s a -d 'show all' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :c" -s F -d 'only show directories' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :c" -s f -d 'show only files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :c" -l sort -r -f -a "name\t''
mtime\t''
atime\t''
size\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :c" -l transform -d 'Lua transform (path, tail) -> (path, display, tail)' -r
complete -c fs -n "__fish_fs_using_subcommand :c" -l tail-sep -d 'Delimiter used to split off the input into a (path, tail) pair' -r
complete -c fs -n "__fish_fs_using_subcommand :c" -l input-sep -d 'Split the stream on this character instead of newlines' -r
complete -c fs -n "__fish_fs_using_subcommand :c" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :c" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :c" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :c" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :c" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :c" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :c" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :c" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :c" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :c" -l cd -d 'print the first match'
complete -c fs -n "__fish_fs_using_subcommand :c" -l help
complete -c fs -n "__fish_fs_using_subcommand :c" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :c" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :c" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :rg" -s h -d 'show hidden files and folders' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -s I -d 'HIDE ignored files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -s a -d 'show all' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -s F -d 'only show directories' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -s f -d 'show only files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -l sort -r -f -a "name\t''
mtime\t''
atime\t''
size\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -s p -l path -d 'Files or directories to search in' -r -F
complete -c fs -n "__fish_fs_using_subcommand :rg" -s A -l after-context -d 'Show NUM lines after each match' -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -s B -l before-context -d 'Show NUM lines before each match' -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -s C -l context -d 'Show NUM lines before and after each match' -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -l one-line -d 'Display each match on a separate line. Alias: `-1`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -l query -d 'initial query' -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :rg" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :rg" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :rg" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :rg" -s i -l ignore-case
complete -c fs -n "__fish_fs_using_subcommand :rg" -s s -l case-sensitive
complete -c fs -n "__fish_fs_using_subcommand :rg" -s S -l smart-case
complete -c fs -n "__fish_fs_using_subcommand :rg" -l fixed-strings -d 'Enable fixed string matching'
complete -c fs -n "__fish_fs_using_subcommand :rg" -l no-fixed-strings -d 'Disable fixed string matching'
complete -c fs -n "__fish_fs_using_subcommand :rg" -l preserve-whitespace -d 'Prepend \' to query start'
complete -c fs -n "__fish_fs_using_subcommand :rg" -l rebase -d 'Execute in the deepest directory common to all given paths'
complete -c fs -n "__fish_fs_using_subcommand :rg" -l filtering
complete -c fs -n "__fish_fs_using_subcommand :rg" -s 1 -l no-heading
complete -c fs -n "__fish_fs_using_subcommand :rg" -l list
complete -c fs -n "__fish_fs_using_subcommand :rg" -l no-read -d 'Don\'t try to read paths from stdin'
complete -c fs -n "__fish_fs_using_subcommand :rg" -l help
complete -c fs -n "__fish_fs_using_subcommand :rg" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :rg" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :rg" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :" -s h -d 'show hidden files and folders' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -s I -d 'HIDE ignored files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -s a -d 'show all' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -s F -d 'only show directories' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -s f -d 'show only files' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -l sort -r -f -a "name\t''
mtime\t''
atime\t''
size\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -s p -l path -d 'Files or directories to search in' -r -F
complete -c fs -n "__fish_fs_using_subcommand :" -s A -l after-context -d 'Show NUM lines after each match' -r
complete -c fs -n "__fish_fs_using_subcommand :" -s B -l before-context -d 'Show NUM lines before each match' -r
complete -c fs -n "__fish_fs_using_subcommand :" -s C -l context -d 'Show NUM lines before and after each match' -r
complete -c fs -n "__fish_fs_using_subcommand :" -l one-line -d 'Display each match on a separate line. Alias: `-1`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -l query -d 'initial query' -r
complete -c fs -n "__fish_fs_using_subcommand :" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :" -s i -l ignore-case
complete -c fs -n "__fish_fs_using_subcommand :" -s s -l case-sensitive
complete -c fs -n "__fish_fs_using_subcommand :" -s S -l smart-case
complete -c fs -n "__fish_fs_using_subcommand :" -l fixed-strings -d 'Enable fixed string matching'
complete -c fs -n "__fish_fs_using_subcommand :" -l no-fixed-strings -d 'Disable fixed string matching'
complete -c fs -n "__fish_fs_using_subcommand :" -l preserve-whitespace -d 'Prepend \' to query start'
complete -c fs -n "__fish_fs_using_subcommand :" -l rebase -d 'Execute in the deepest directory common to all given paths'
complete -c fs -n "__fish_fs_using_subcommand :" -l filtering
complete -c fs -n "__fish_fs_using_subcommand :" -s 1 -l no-heading
complete -c fs -n "__fish_fs_using_subcommand :" -l list
complete -c fs -n "__fish_fs_using_subcommand :" -l no-read -d 'Don\'t try to read paths from stdin'
complete -c fs -n "__fish_fs_using_subcommand :" -l help
complete -c fs -n "__fish_fs_using_subcommand :" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l help
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "colors"
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "liza" -d 'List directory (eza wrapper)'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "shell" -d 'Dump the initialization code for your shell'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "lessfilter" -d 'Context and preset dependent file handler'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "pager"
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "bump" -d 'Bump history entries'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "trash" -d 'Trash files with timed fallback prompts'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "show-binds" -d 'Show binds'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "types" -d 'List mappings supported by the --type parameter'
complete -c fs -n "__fish_fs_using_subcommand :tool; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "diskspace" -d 'Disk usage: compute directory sizes concurrently and print them'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from colors" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from liza" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from shell" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from lessfilter" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from pager" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from bump" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from trash" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from show-binds" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from types" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :tool; and __fish_seen_subcommand_from diskspace" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l help
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "colors"
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "liza" -d 'List directory (eza wrapper)'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "shell" -d 'Dump the initialization code for your shell'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "lessfilter" -d 'Context and preset dependent file handler'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "pager"
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "bump" -d 'Bump history entries'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "trash" -d 'Trash files with timed fallback prompts'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "show-binds" -d 'Show binds'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "types" -d 'List mappings supported by the --type parameter'
complete -c fs -n "__fish_fs_using_subcommand :t; and not __fish_seen_subcommand_from colors liza shell lessfilter pager bump trash show-binds types diskspace" -a "diskspace" -d 'Disk usage: compute directory sizes concurrently and print them'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from colors" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from liza" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from shell" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from lessfilter" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from pager" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from bump" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from trash" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from show-binds" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from types" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :t; and __fish_seen_subcommand_from diskspace" -l alt-accept
complete -c fs -n "__fish_fs_using_subcommand :info" -l sort -d 'history sort order' -r -f -a "name\t''
mtime\t''
atime\t''
size\t''
none\t''"
complete -c fs -n "__fish_fs_using_subcommand :info" -s l -l limit -d 'maximum history entries to display' -r
complete -c fs -n "__fish_fs_using_subcommand :info" -l override -d 'config override' -r
complete -c fs -n "__fish_fs_using_subcommand :info" -l config -d 'config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :info" -l mm-config -d 'matchmaker config path' -r -F
complete -c fs -n "__fish_fs_using_subcommand :info" -l style -r -f -a "icons\t''
icon-colors\t''
colors\t''
none\t''
all\t''
auto\t''"
complete -c fs -n "__fish_fs_using_subcommand :info" -l fullscreen -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :info" -l lock-prompt -d 'See `interface.prompt_locking`' -r -f -a "true\t''
false\t''"
complete -c fs -n "__fish_fs_using_subcommand :info" -l output-sep -d 'Separator printed after each result' -r
complete -c fs -n "__fish_fs_using_subcommand :info" -l format -d 'Output template for printed results' -r
complete -c fs -n "__fish_fs_using_subcommand :info" -l opener -d 'Program used to open files on accept' -r
complete -c fs -n "__fish_fs_using_subcommand :info" -s m -l minimal -d 'Don\'t print decorations'
complete -c fs -n "__fish_fs_using_subcommand :info" -l help
complete -c fs -n "__fish_fs_using_subcommand :info" -s q -d 'Reduce the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :info" -s v -d 'Increase the verbosity level'
complete -c fs -n "__fish_fs_using_subcommand :info" -l alt-accept
