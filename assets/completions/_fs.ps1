
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
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'sort')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'show hidden files and folders')
            [CompletionResult]::new('-I', '-I ', [CompletionResultType]::ParameterName, 'HIDE ignored files')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'show all')
            [CompletionResult]::new('-F', '-F ', [CompletionResultType]::ParameterName, 'only show directories')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'show only files')
            [CompletionResult]::new('-t', '-t', [CompletionResultType]::ParameterName, 'restrict search to certain file types and extensions (use `:t types` to list)')
            [CompletionResult]::new('--types', '--types', [CompletionResultType]::ParameterName, 'restrict search to certain file types and extensions (use `:t types` to list)')
            [CompletionResult]::new('--dump-config', '--dump-config', [CompletionResultType]::ParameterName, 'Dump the main config and any other missing configuration files to default locations: If the output was detected to have been redirected, this prints the main configuration. Otherwise, this will OVERWRITE your main config.')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            [CompletionResult]::new('--cd', '--cd', [CompletionResultType]::ParameterName, 'print the first match')
            [CompletionResult]::new('--no-read', '--no-read', [CompletionResultType]::ParameterName, 'Never stream input from stdin')
            [CompletionResult]::new('--reset-visibility', '--reset-visibility', [CompletionResultType]::ParameterName, 'reset-visibility')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
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
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'initial query')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
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
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'initial query')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
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
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:dir' {
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'history sort order')
            [CompletionResult]::new('-l', '-l', [CompletionResultType]::ParameterName, 'l')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--initial-input', '--initial-input', [CompletionResultType]::ParameterName, 'initial-input')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--cd', '--cd', [CompletionResultType]::ParameterName, 'print the first match')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:fd' {
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'sort')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'show hidden files and folders')
            [CompletionResult]::new('-I', '-I ', [CompletionResultType]::ParameterName, 'HIDE ignored files')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'show all')
            [CompletionResult]::new('-F', '-F ', [CompletionResultType]::ParameterName, 'only show directories')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'show only files')
            [CompletionResult]::new('-t', '-t', [CompletionResultType]::ParameterName, 'restrict search to certain file types and extensions (use `:t types` to list)')
            [CompletionResult]::new('--types', '--types', [CompletionResultType]::ParameterName, 'restrict search to certain file types and extensions (use `:t types` to list)')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--cd', '--cd', [CompletionResultType]::ParameterName, 'print the first match')
            [CompletionResult]::new('--no-read', '--no-read', [CompletionResultType]::ParameterName, 'Never stream input from stdin')
            [CompletionResult]::new('--reset-visibility', '--reset-visibility', [CompletionResultType]::ParameterName, 'reset-visibility')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;::' {
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'sort')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'show hidden files and folders')
            [CompletionResult]::new('-I', '-I ', [CompletionResultType]::ParameterName, 'HIDE ignored files')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'show all')
            [CompletionResult]::new('-F', '-F ', [CompletionResultType]::ParameterName, 'only show directories')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'show only files')
            [CompletionResult]::new('-t', '-t', [CompletionResultType]::ParameterName, 'restrict search to certain file types and extensions (use `:t types` to list)')
            [CompletionResult]::new('--types', '--types', [CompletionResultType]::ParameterName, 'restrict search to certain file types and extensions (use `:t types` to list)')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--cd', '--cd', [CompletionResultType]::ParameterName, 'print the first match')
            [CompletionResult]::new('--no-read', '--no-read', [CompletionResultType]::ParameterName, 'Never stream input from stdin')
            [CompletionResult]::new('--reset-visibility', '--reset-visibility', [CompletionResultType]::ParameterName, 'reset-visibility')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:rg' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'show hidden files and folders')
            [CompletionResult]::new('-I', '-I ', [CompletionResultType]::ParameterName, 'HIDE ignored files')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'show all')
            [CompletionResult]::new('-F', '-F ', [CompletionResultType]::ParameterName, 'only show directories')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'show only files')
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'sort')
            [CompletionResult]::new('-p', '-p', [CompletionResultType]::ParameterName, 'Files or directories to search in')
            [CompletionResult]::new('--path', '--path', [CompletionResultType]::ParameterName, 'Files or directories to search in')
            [CompletionResult]::new('-A', '-A ', [CompletionResultType]::ParameterName, 'Show NUM lines after each match')
            [CompletionResult]::new('--after-context', '--after-context', [CompletionResultType]::ParameterName, 'Show NUM lines after each match')
            [CompletionResult]::new('-B', '-B ', [CompletionResultType]::ParameterName, 'Show NUM lines before each match')
            [CompletionResult]::new('--before-context', '--before-context', [CompletionResultType]::ParameterName, 'Show NUM lines before each match')
            [CompletionResult]::new('-C', '-C ', [CompletionResultType]::ParameterName, 'Show NUM lines before and after each match')
            [CompletionResult]::new('--context', '--context', [CompletionResultType]::ParameterName, 'Show NUM lines before and after each match')
            [CompletionResult]::new('--one-line', '--one-line', [CompletionResultType]::ParameterName, 'Display each match on a separate line. Alias: `-1`')
            [CompletionResult]::new('--query', '--query', [CompletionResultType]::ParameterName, 'initial query')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('-i', '-i', [CompletionResultType]::ParameterName, 'i')
            [CompletionResult]::new('--ignore-case', '--ignore-case', [CompletionResultType]::ParameterName, 'ignore-case')
            [CompletionResult]::new('-s', '-s', [CompletionResultType]::ParameterName, 's')
            [CompletionResult]::new('--case-sensitive', '--case-sensitive', [CompletionResultType]::ParameterName, 'case-sensitive')
            [CompletionResult]::new('-S', '-S ', [CompletionResultType]::ParameterName, 'S')
            [CompletionResult]::new('--smart-case', '--smart-case', [CompletionResultType]::ParameterName, 'smart-case')
            [CompletionResult]::new('--fixed-strings', '--fixed-strings', [CompletionResultType]::ParameterName, 'Enable fixed string matching')
            [CompletionResult]::new('--no-fixed-strings', '--no-fixed-strings', [CompletionResultType]::ParameterName, 'Disable fixed string matching')
            [CompletionResult]::new('--preserve-whitespace', '--preserve-whitespace', [CompletionResultType]::ParameterName, 'Prepend '' to query start')
            [CompletionResult]::new('--rebase', '--rebase', [CompletionResultType]::ParameterName, 'Execute in the deepest directory common to all given paths')
            [CompletionResult]::new('--filtering', '--filtering', [CompletionResultType]::ParameterName, 'filtering')
            [CompletionResult]::new('-1', '-1', [CompletionResultType]::ParameterName, '1')
            [CompletionResult]::new('--no-heading', '--no-heading', [CompletionResultType]::ParameterName, 'no-heading')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--no-read', '--no-read', [CompletionResultType]::ParameterName, 'Don''t try to read paths from stdin')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:' {
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'show hidden files and folders')
            [CompletionResult]::new('-I', '-I ', [CompletionResultType]::ParameterName, 'HIDE ignored files')
            [CompletionResult]::new('-a', '-a', [CompletionResultType]::ParameterName, 'show all')
            [CompletionResult]::new('-F', '-F ', [CompletionResultType]::ParameterName, 'only show directories')
            [CompletionResult]::new('-f', '-f', [CompletionResultType]::ParameterName, 'show only files')
            [CompletionResult]::new('--sort', '--sort', [CompletionResultType]::ParameterName, 'sort')
            [CompletionResult]::new('-p', '-p', [CompletionResultType]::ParameterName, 'Files or directories to search in')
            [CompletionResult]::new('--path', '--path', [CompletionResultType]::ParameterName, 'Files or directories to search in')
            [CompletionResult]::new('-A', '-A ', [CompletionResultType]::ParameterName, 'Show NUM lines after each match')
            [CompletionResult]::new('--after-context', '--after-context', [CompletionResultType]::ParameterName, 'Show NUM lines after each match')
            [CompletionResult]::new('-B', '-B ', [CompletionResultType]::ParameterName, 'Show NUM lines before each match')
            [CompletionResult]::new('--before-context', '--before-context', [CompletionResultType]::ParameterName, 'Show NUM lines before each match')
            [CompletionResult]::new('-C', '-C ', [CompletionResultType]::ParameterName, 'Show NUM lines before and after each match')
            [CompletionResult]::new('--context', '--context', [CompletionResultType]::ParameterName, 'Show NUM lines before and after each match')
            [CompletionResult]::new('--one-line', '--one-line', [CompletionResultType]::ParameterName, 'Display each match on a separate line. Alias: `-1`')
            [CompletionResult]::new('--query', '--query', [CompletionResultType]::ParameterName, 'initial query')
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('-i', '-i', [CompletionResultType]::ParameterName, 'i')
            [CompletionResult]::new('--ignore-case', '--ignore-case', [CompletionResultType]::ParameterName, 'ignore-case')
            [CompletionResult]::new('-s', '-s', [CompletionResultType]::ParameterName, 's')
            [CompletionResult]::new('--case-sensitive', '--case-sensitive', [CompletionResultType]::ParameterName, 'case-sensitive')
            [CompletionResult]::new('-S', '-S ', [CompletionResultType]::ParameterName, 'S')
            [CompletionResult]::new('--smart-case', '--smart-case', [CompletionResultType]::ParameterName, 'smart-case')
            [CompletionResult]::new('--fixed-strings', '--fixed-strings', [CompletionResultType]::ParameterName, 'Enable fixed string matching')
            [CompletionResult]::new('--no-fixed-strings', '--no-fixed-strings', [CompletionResultType]::ParameterName, 'Disable fixed string matching')
            [CompletionResult]::new('--preserve-whitespace', '--preserve-whitespace', [CompletionResultType]::ParameterName, 'Prepend '' to query start')
            [CompletionResult]::new('--rebase', '--rebase', [CompletionResultType]::ParameterName, 'Execute in the deepest directory common to all given paths')
            [CompletionResult]::new('--filtering', '--filtering', [CompletionResultType]::ParameterName, 'filtering')
            [CompletionResult]::new('-1', '-1', [CompletionResultType]::ParameterName, '1')
            [CompletionResult]::new('--no-heading', '--no-heading', [CompletionResultType]::ParameterName, 'no-heading')
            [CompletionResult]::new('--list', '--list', [CompletionResultType]::ParameterName, 'list')
            [CompletionResult]::new('--no-read', '--no-read', [CompletionResultType]::ParameterName, 'Don''t try to read paths from stdin')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:tool' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            [CompletionResult]::new('colors', 'colors', [CompletionResultType]::ParameterValue, 'colors')
            [CompletionResult]::new('liza', 'liza', [CompletionResultType]::ParameterValue, 'List directory (eza wrapper)')
            [CompletionResult]::new('shell', 'shell', [CompletionResultType]::ParameterValue, 'Dump the initialization code for your shell')
            [CompletionResult]::new('lessfilter', 'lessfilter', [CompletionResultType]::ParameterValue, 'Context and preset dependent file handler')
            [CompletionResult]::new('pager', 'pager', [CompletionResultType]::ParameterValue, 'pager')
            [CompletionResult]::new('bump', 'bump', [CompletionResultType]::ParameterValue, 'Bump history entries')
            [CompletionResult]::new('trash', 'trash', [CompletionResultType]::ParameterValue, 'Trash files with timed fallback prompts')
            [CompletionResult]::new('show-binds', 'show-binds', [CompletionResultType]::ParameterValue, 'Show binds')
            [CompletionResult]::new('types', 'types', [CompletionResultType]::ParameterValue, 'List mappings supported by the --type parameter')
            break
        }
        'fs;:t' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            [CompletionResult]::new('colors', 'colors', [CompletionResultType]::ParameterValue, 'colors')
            [CompletionResult]::new('liza', 'liza', [CompletionResultType]::ParameterValue, 'List directory (eza wrapper)')
            [CompletionResult]::new('shell', 'shell', [CompletionResultType]::ParameterValue, 'Dump the initialization code for your shell')
            [CompletionResult]::new('lessfilter', 'lessfilter', [CompletionResultType]::ParameterValue, 'Context and preset dependent file handler')
            [CompletionResult]::new('pager', 'pager', [CompletionResultType]::ParameterValue, 'pager')
            [CompletionResult]::new('bump', 'bump', [CompletionResultType]::ParameterValue, 'Bump history entries')
            [CompletionResult]::new('trash', 'trash', [CompletionResultType]::ParameterValue, 'Trash files with timed fallback prompts')
            [CompletionResult]::new('show-binds', 'show-binds', [CompletionResultType]::ParameterValue, 'Show binds')
            [CompletionResult]::new('types', 'types', [CompletionResultType]::ParameterValue, 'List mappings supported by the --type parameter')
            break
        }
        'fs;:tool;colors' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:t;colors' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:tool;liza' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:t;liza' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:tool;shell' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:t;shell' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:tool;lessfilter' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:t;lessfilter' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:tool;pager' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:t;pager' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:tool;bump' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:t;bump' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:tool;trash' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:t;trash' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:tool;show-binds' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:t;show-binds' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:tool;types' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
        'fs;:t;types' {
            [CompletionResult]::new('--verbosity', '--verbosity', [CompletionResultType]::ParameterName, 'verbosity')
            [CompletionResult]::new('--override', '--override', [CompletionResultType]::ParameterName, 'config override')
            [CompletionResult]::new('--config', '--config', [CompletionResultType]::ParameterName, 'config path')
            [CompletionResult]::new('--mm-config', '--mm-config', [CompletionResultType]::ParameterName, 'matchmaker config path')
            [CompletionResult]::new('--style', '--style', [CompletionResultType]::ParameterName, 'style')
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
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
            [CompletionResult]::new('--fullscreen', '--fullscreen', [CompletionResultType]::ParameterName, 'fullscreen')
            [CompletionResult]::new('--enter-prompt', '--enter-prompt', [CompletionResultType]::ParameterName, 'enter-prompt')
            [CompletionResult]::new('-m', '-m', [CompletionResultType]::ParameterName, 'Don''t print decorations')
            [CompletionResult]::new('--minimal', '--minimal', [CompletionResultType]::ParameterName, 'Don''t print decorations')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'help')
            [CompletionResult]::new('--alt-accept', '--alt-accept', [CompletionResultType]::ParameterName, 'alt-accept')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
