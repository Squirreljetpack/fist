
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'fs' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'fs'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'fs' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--dump-config', '--dump-config', [CompletionResultType]::ParameterName, 'Dump the main config and any other missing configuration files to default locations: If the output was detected to have been redirected, this prints the main configuration. Otherwise, this WILL OVERWRITE your main config.')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new(':open', ':open', [CompletionResultType]::ParameterValue, 'Launch apps and files')
            [CompletionResult]::new(':o', ':o', [CompletionResultType]::ParameterValue, 'Launch apps and files')
            [CompletionResult]::new(':file', ':file', [CompletionResultType]::ParameterValue, 'Recent files')
            [CompletionResult]::new(':dir', ':dir', [CompletionResultType]::ParameterValue, 'Recent folders')
            [CompletionResult]::new(':fd', ':fd', [CompletionResultType]::ParameterValue, 'Find and browse. (Default)')
            [CompletionResult]::new('::', '::', [CompletionResultType]::ParameterValue, 'Find and browse. (Default)')
            [CompletionResult]::new(':rg', ':rg', [CompletionResultType]::ParameterValue, 'Full text search')
            [CompletionResult]::new(':', ':', [CompletionResultType]::ParameterValue, 'Full text search')
            [CompletionResult]::new(':tool', ':tool', [CompletionResultType]::ParameterValue, 'Plugins and utilities')
            [CompletionResult]::new(':t', ':t', [CompletionResultType]::ParameterValue, 'Plugins and utilities')
            [CompletionResult]::new(':info', ':info', [CompletionResultType]::ParameterValue, 'Stats and database records')
            break
        }
        'fs;:open' {
            [CompletionResult]::new('-w', '-w', [CompletionResultType]::ParameterName, 'app to open files with')
            [CompletionResult]::new('--with', '--with', [CompletionResultType]::ParameterName, 'app to open files with')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'initial query')
            break
        }
        'fs;:o' {
            [CompletionResult]::new('-w', '-w', [CompletionResultType]::ParameterName, 'app to open files with')
            [CompletionResult]::new('--with', '--with', [CompletionResultType]::ParameterName, 'app to open files with')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'initial query')
            break
        }
        'fs;:file' {
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'history sort order')
            [CompletionResult]::new('-l', '-l', [CompletionResultType]::ParameterName, 'l')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--query', '--query', [CompletionResultType]::ParameterName, 'initial query')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            break
        }
        'fs;:dir' {
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'history sort order')
            [CompletionResult]::new('-l', '-l', [CompletionResultType]::ParameterName, 'l')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--cd', '--cd', [CompletionResultType]::ParameterName, 'print the first match')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            break
        }
        'fs;:fd' {
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'sort')
            [CompletionResult]::new('-t', '-t', [CompletionResultType]::ParameterName, 'restrict search to certain file types and extensions (use `:t types` to list)')
            [CompletionResult]::new('--types', '--types', [CompletionResultType]::ParameterName, 'restrict search to certain file types and extensions (use `:t types` to list)')
            [CompletionResult]::new('--list-fmt', '--list-fmt', [CompletionResultType]::ParameterName, 'Template to format the list output as')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'show hidden files and folders')
            [CompletionResult]::new('-I', '-I ', [CompletionResultType]::ParameterName, 'HIDE ignored files')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'show all')
            [CompletionResult]::new('-F', '-F ', [CompletionResultType]::ParameterName, 'only show directories')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'show only files')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--no-read', '--no-read', [CompletionResultType]::ParameterName, 'Never stream input from stdin')
            [CompletionResult]::new('--cd', '--cd', [CompletionResultType]::ParameterName, 'print the first match')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            break
        }
        'fs;::' {
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'sort')
            [CompletionResult]::new('-t', '-t', [CompletionResultType]::ParameterName, 'restrict search to certain file types and extensions (use `:t types` to list)')
            [CompletionResult]::new('--types', '--types', [CompletionResultType]::ParameterName, 'restrict search to certain file types and extensions (use `:t types` to list)')
            [CompletionResult]::new('--list-fmt', '--list-fmt', [CompletionResultType]::ParameterName, 'Template to format the list output as')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'show hidden files and folders')
            [CompletionResult]::new('-I', '-I ', [CompletionResultType]::ParameterName, 'HIDE ignored files')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'show all')
            [CompletionResult]::new('-F', '-F ', [CompletionResultType]::ParameterName, 'only show directories')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'show only files')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--no-read', '--no-read', [CompletionResultType]::ParameterName, 'Never stream input from stdin')
            [CompletionResult]::new('--cd', '--cd', [CompletionResultType]::ParameterName, 'print the first match')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            break
        }
        'fs;:rg' {
            [CompletionResult]::new('--query', '--query', [CompletionResultType]::ParameterName, 'initial query')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'show hidden files and folders')
            [CompletionResult]::new('-I', '-I ', [CompletionResultType]::ParameterName, 'HIDE ignored files')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'show all')
            [CompletionResult]::new('-F', '-F ', [CompletionResultType]::ParameterName, 'only show directories')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'show only files')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            break
        }
        'fs;:' {
            [CompletionResult]::new('--query', '--query', [CompletionResultType]::ParameterName, 'initial query')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'show hidden files and folders')
            [CompletionResult]::new('-I', '-I ', [CompletionResultType]::ParameterName, 'HIDE ignored files')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'show all')
            [CompletionResult]::new('-F', '-F ', [CompletionResultType]::ParameterName, 'only show directories')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'show only files')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            break
        }
        'fs;:tool' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('colors', 'colors', [CompletionResultType]::ParameterValue, 'colors')
            [CompletionResult]::new('liza', 'liza', [CompletionResultType]::ParameterValue, 'List directory (eza wrapper)')
            [CompletionResult]::new('shell', 'shell', [CompletionResultType]::ParameterValue, 'Dump the initialization code for your shell')
            [CompletionResult]::new('lessfilter', 'lessfilter', [CompletionResultType]::ParameterValue, 'Context and preset dependent file handler')
            [CompletionResult]::new('bump', 'bump', [CompletionResultType]::ParameterValue, 'Bump history entries')
            [CompletionResult]::new('types', 'types', [CompletionResultType]::ParameterValue, 'List mappings supported by the --type parameter')
            break
        }
        'fs;:t' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('colors', 'colors', [CompletionResultType]::ParameterValue, 'colors')
            [CompletionResult]::new('liza', 'liza', [CompletionResultType]::ParameterValue, 'List directory (eza wrapper)')
            [CompletionResult]::new('shell', 'shell', [CompletionResultType]::ParameterValue, 'Dump the initialization code for your shell')
            [CompletionResult]::new('lessfilter', 'lessfilter', [CompletionResultType]::ParameterValue, 'Context and preset dependent file handler')
            [CompletionResult]::new('bump', 'bump', [CompletionResultType]::ParameterValue, 'Bump history entries')
            [CompletionResult]::new('types', 'types', [CompletionResultType]::ParameterValue, 'List mappings supported by the --type parameter')
            break
        }
        'fs;:tool;colors' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            break
        }
        'fs;:t;colors' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            break
        }
        'fs;:tool;liza' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            break
        }
        'fs;:t;liza' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            break
        }
        'fs;:tool;shell' {
            [CompletionResult]::new('--z-name', '--z-name', [CompletionResultType]::ParameterName, 'Name for jump function')
            [CompletionResult]::new('--z-dot-args', '--z-dot-args', [CompletionResultType]::ParameterName, 'Arguments passed to `fs ::` when z is invoked with a trailing `.`')
            [CompletionResult]::new('--z-slash-args', '--z-slash-args', [CompletionResultType]::ParameterName, 'Arguments passed to `fs ::` when z is invoked with a trailing `./`')
            [CompletionResult]::new('--z-sort', '--z-sort', [CompletionResultType]::ParameterName, 'Default sort order for the interactive jump menu')
            [CompletionResult]::new('--open-name', '--open-name', [CompletionResultType]::ParameterName, 'Name for open function')
            [CompletionResult]::new('--open-cmd', '--open-cmd', [CompletionResultType]::ParameterName, 'Command used by open function')
            [CompletionResult]::new('--dir-widget-bind', '--dir-widget-bind', [CompletionResultType]::ParameterName, 'Bind for the directory widget')
            [CompletionResult]::new('--file-widget-bind', '--file-widget-bind', [CompletionResultType]::ParameterName, 'Bind for the directory widget')
            [CompletionResult]::new('--rg-widget-bind', '--rg-widget-bind', [CompletionResultType]::ParameterName, 'Bind for the directory widget')
            [CompletionResult]::new('--file-open-cmd', '--file-open-cmd', [CompletionResultType]::ParameterName, 'file-open-cmd')
            [CompletionResult]::new('--rg-open-cmd', '--rg-open-cmd', [CompletionResultType]::ParameterName, 'rg-open-cmd')
            [CompletionResult]::new('--dir-widget-args', '--dir-widget-args', [CompletionResultType]::ParameterName, 'Arguments passed to `fs ::` when dir widget is invoked')
            [CompletionResult]::new('--file-widget-args', '--file-widget-args', [CompletionResultType]::ParameterName, 'Arguments passed to `fs ::` when file widget is invoked')
            [CompletionResult]::new('--rg-widget-args', '--rg-widget-args', [CompletionResultType]::ParameterName, 'Arguments passed to `fs :` when rg widget is invoked')
            [CompletionResult]::new('--shell', '--shell', [CompletionResultType]::ParameterName, 'shell')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--aliases', '--aliases', [CompletionResultType]::ParameterName, 'aliases')
            break
        }
        'fs;:t;shell' {
            [CompletionResult]::new('--z-name', '--z-name', [CompletionResultType]::ParameterName, 'Name for jump function')
            [CompletionResult]::new('--z-dot-args', '--z-dot-args', [CompletionResultType]::ParameterName, 'Arguments passed to `fs ::` when z is invoked with a trailing `.`')
            [CompletionResult]::new('--z-slash-args', '--z-slash-args', [CompletionResultType]::ParameterName, 'Arguments passed to `fs ::` when z is invoked with a trailing `./`')
            [CompletionResult]::new('--z-sort', '--z-sort', [CompletionResultType]::ParameterName, 'Default sort order for the interactive jump menu')
            [CompletionResult]::new('--open-name', '--open-name', [CompletionResultType]::ParameterName, 'Name for open function')
            [CompletionResult]::new('--open-cmd', '--open-cmd', [CompletionResultType]::ParameterName, 'Command used by open function')
            [CompletionResult]::new('--dir-widget-bind', '--dir-widget-bind', [CompletionResultType]::ParameterName, 'Bind for the directory widget')
            [CompletionResult]::new('--file-widget-bind', '--file-widget-bind', [CompletionResultType]::ParameterName, 'Bind for the directory widget')
            [CompletionResult]::new('--rg-widget-bind', '--rg-widget-bind', [CompletionResultType]::ParameterName, 'Bind for the directory widget')
            [CompletionResult]::new('--file-open-cmd', '--file-open-cmd', [CompletionResultType]::ParameterName, 'file-open-cmd')
            [CompletionResult]::new('--rg-open-cmd', '--rg-open-cmd', [CompletionResultType]::ParameterName, 'rg-open-cmd')
            [CompletionResult]::new('--dir-widget-args', '--dir-widget-args', [CompletionResultType]::ParameterName, 'Arguments passed to `fs ::` when dir widget is invoked')
            [CompletionResult]::new('--file-widget-args', '--file-widget-args', [CompletionResultType]::ParameterName, 'Arguments passed to `fs ::` when file widget is invoked')
            [CompletionResult]::new('--rg-widget-args', '--rg-widget-args', [CompletionResultType]::ParameterName, 'Arguments passed to `fs :` when rg widget is invoked')
            [CompletionResult]::new('--shell', '--shell', [CompletionResultType]::ParameterName, 'shell')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--aliases', '--aliases', [CompletionResultType]::ParameterName, 'aliases')
            break
        }
        'fs;:tool;lessfilter' {
            [CompletionResult]::new('--header', '--header', [CompletionResultType]::ParameterName, 'header')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            break
        }
        'fs;:t;lessfilter' {
            [CompletionResult]::new('--header', '--header', [CompletionResultType]::ParameterName, 'header')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            break
        }
        'fs;:tool;bump' {
            [CompletionResult]::new('-g', '-g', [CompletionResultType]::ParameterName, 'glob pattern to bump')
            [CompletionResult]::new('--glob', '--glob', [CompletionResultType]::ParameterName, 'glob pattern to bump')
            [CompletionResult]::new('-c', '-c', [CompletionResultType]::ParameterName, 'amount to bump by, 0 to clear')
            [CompletionResult]::new('--count', '--count', [CompletionResultType]::ParameterName, 'amount to bump by, 0 to clear')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--reset', '--reset', [CompletionResultType]::ParameterName, 'reset the database')
            break
        }
        'fs;:t;bump' {
            [CompletionResult]::new('-g', '-g', [CompletionResultType]::ParameterName, 'glob pattern to bump')
            [CompletionResult]::new('--glob', '--glob', [CompletionResultType]::ParameterName, 'glob pattern to bump')
            [CompletionResult]::new('-c', '-c', [CompletionResultType]::ParameterName, 'amount to bump by, 0 to clear')
            [CompletionResult]::new('--count', '--count', [CompletionResultType]::ParameterName, 'amount to bump by, 0 to clear')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--reset', '--reset', [CompletionResultType]::ParameterName, 'reset the database')
            break
        }
        'fs;:tool;types' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            break
        }
        'fs;:t;types' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            break
        }
        'fs;:info' {
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'history sort order')
            [CompletionResult]::new('-l', '-l', [CompletionResultType]::ParameterName, 'maximum history entries to display')
            [CompletionResult]::new('--limit', '--limit', [CompletionResultType]::ParameterName, 'maximum history entries to display')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('-m', '-m', [CompletionResultType]::ParameterName, 'Don''t print decorations')
            [CompletionResult]::new('--minimal', '--minimal', [CompletionResultType]::ParameterName, 'Don''t print decorations')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
