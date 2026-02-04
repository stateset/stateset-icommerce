#!/usr/bin/env node

/**
 * StateSet CLI - Shell Completion Generator
 *
 * Generates shell completion scripts for bash and zsh.
 *
 * Usage:
 *   stateset-completion bash > ~/.stateset/completions.bash
 *   stateset-completion zsh > ~/.stateset/_stateset
 *   stateset-completion fish > ~/.config/fish/completions/stateset.fish
 *
 * Installation:
 *   # Bash: Add to ~/.bashrc
 *   source ~/.stateset/completions.bash
 *
 *   # Zsh: Add to ~/.zshrc
 *   fpath=(~/.stateset $fpath)
 *   autoload -Uz compinit && compinit
 *
 *   # Fish: Already loaded from completions directory
 */

const HELP = `
StateSet CLI - Shell Completion Generator

USAGE:
  stateset-completion <shell>

SHELLS:
  bash    Generate bash completion script
  zsh     Generate zsh completion script
  fish    Generate fish completion script

INSTALLATION:

  Bash:
  stateset-completion bash > ~/.stateset/completions.bash
  echo 'source ~/.stateset/completions.bash' >> ~/.bashrc

  Zsh:
    mkdir -p ~/.stateset
    stateset-completion zsh > ~/.stateset/_stateset
    echo 'fpath=(~/.stateset $fpath)' >> ~/.zshrc
    echo 'autoload -Uz compinit && compinit' >> ~/.zshrc

  Fish:
    stateset-completion fish > ~/.config/fish/completions/stateset.fish

EXAMPLES:
  stateset-completion bash
  stateset-completion zsh > /usr/local/share/zsh/site-functions/_stateset
`;

// Command structure for completions
const COMMANDS = {
  'ss': {
    options: ['--db', '--apply', '--agent', '--profile', '--model', '--provider', '--think', '--stream', '--budget', '--memory', '--no-memory', '--x402', '--resume', '--json', '--format', '--output', '--verbose', '--stats', '--yes', '--quiet', '--stdin', '--batch', '--parallel', '--help', '--version'],
    description: 'Shorthand for stateset'
  },
  'stateset': {
    options: ['--db', '--apply', '--agent', '--profile', '--model', '--provider', '--think', '--stream', '--budget', '--memory', '--no-memory', '--x402', '--resume', '--json', '--format', '--output', '--verbose', '--stats', '--yes', '--quiet', '--stdin', '--batch', '--parallel', '--help', '--version'],
    description: 'AI-powered commerce CLI'
  },
  'stateset-direct': {
    resources: {
      customers: ['list', 'get', 'create', 'count', 'search'],
      orders: ['list', 'get', 'ship', 'cancel', 'count', 'status', 'pending', 'recent'],
      products: ['list', 'get', 'variant', 'variants', 'count', 'search'],
      inventory: ['list', 'stock', 'adjust', 'create', 'low', 'reserve', 'release'],
      returns: ['list', 'get', 'approve', 'reject', 'count', 'pending', 'create', 'stats'],
      vector: ['search', 'index', 'index-all', 'stats', 'clear', 'clear-all']
    },
    aliases: {
      c: 'customers', o: 'orders', p: 'products', i: 'inventory', r: 'returns', v: 'vector',
      cust: 'customers', ord: 'orders', prod: 'products', inv: 'inventory', ret: 'returns', vec: 'vector'
    },
    options: ['--db', '--apply', '--json', '--format', '--output', '--yes', '--help']
  },
  'stateset-chat': {
    options: ['--db', '--apply', '--model', '--provider', '--think', '--stream', '--budget', '--memory', '--no-memory', '--x402', '--verbose', '--yes', '--help'],
    description: 'Interactive REPL'
  },
  'stateset-doctor': {
    options: ['--db', '--verbose', '--json', '--output', '--checks', '--fix', '--help'],
    description: 'Health check & diagnostics'
  },
  'stateset-config': {
    subcommands: ['set-key', 'show-keys', 'list', 'show', 'create', 'use', 'set', 'get', 'path'],
    options: ['--profile', '--json', '--output', '--help']
  },
  'stateset-sync': {
    subcommands: ['init', 'status', 'push', 'pull', 'verify', 'conflicts', 'resolve', 'rebase', 'history', 'keys:generate', 'keys:list', 'keys:register', 'keys:rotate', 'keys:export', 'keys:policy', 'keys:expiry', 'keys:batch-rotate', 'groups:create', 'groups:list', 'groups:show', 'groups:add-member', 'groups:remove-member', 'groups:delete', 'groups:refresh-key', 'groups:my-groups'],
    options: ['--db', '--json', '--output', '--help']
  },
  'stateset-pay': {
    options: ['--to', '--amount', '--chain', '--token', '--agent', '--order', '--customer', '--memo', '--wallet', '--balance', '--chains', '--apply', '--json', '--output', '--yes', '--help', '--version']
  },
  'stateset-slack': {
    options: ['--db', '--apply', '--model', '--max-turns', '--agent', '--allow', '--verbose', '--help']
  },
  'stateset-discord': {
    options: ['--db', '--apply', '--model', '--max-turns', '--agent', '--allow', '--mention-only', '--verbose', '--help']
  },
  'stateset-telegram': {
    options: ['--db', '--apply', '--model', '--max-turns', '--agent', '--allow', '--verbose', '--help']
  },
  'stateset-whatsapp': {
    options: ['--db', '--apply', '--model', '--max-turns', '--agent', '--allow', '--groups', '--auth-dir', '--reset', '--verbose', '--help']
  },
  'stateset-signal': {
    options: ['--db', '--apply', '--model', '--max-turns', '--agent', '--allow', '--phone', '--socket', '--verbose', '--help']
  },
  'stateset-google-chat': {
    options: ['--db', '--apply', '--model', '--max-turns', '--agent', '--allow', '--subscription', '--verbose', '--help']
  },
  'stateset-autonomous': {
    subcommands: ['start', 'status', 'init', 'jobs'],
    options: ['--db', '--store', '--port', '--no-webhooks', '--no-scheduler', '--no-workflows', '--no-policies', '--no-approvals', '--init-defaults', '--notify-config', '--force', '--status', '--enabled', '--disabled', '--json', '--output', '--verbose', '--help']
  },
  'stateset-x402': {
    subcommands: ['init'],
    options: ['--sequencer-url', '--tenant-id', '--store-id', '--agent-id', '--network', '--payer-address', '--config-dir', '--config-file', '--agent-key-id', '--budget-per-call', '--budget-daily', '--starting-balance', '--max-amount', '--api-key', '--jwt', '--json', '--output', '--force', '--help', '--version']
  },
  'stateset-x402-mcp': {
    options: ['--config-dir', '--help', '--version']
  },
  'stateset-install-service': {
    options: ['--dry-run', '--uninstall', '--json', '--output', '--help']
  }
};

function generateBashCompletion() {
  return `# StateSet CLI Bash Completion
# Generated by stateset-completion

_stateset_complete() {
    local cur prev words cword
    _init_completion || return

    local commands="stateset ss stateset-direct stateset-chat stateset-doctor stateset-config stateset-sync stateset-pay stateset-checkout stateset-orders stateset-inventory stateset-returns stateset-analytics stateset-promotions stateset-subscriptions stateset-create stateset-events stateset-channels stateset-daemon stateset-skills stateset-x402 stateset-x402-mcp stateset-install-service stateset-autonomous stateset-slack stateset-discord stateset-telegram stateset-whatsapp stateset-signal stateset-google-chat stateset-manufacturing stateset-payments stateset-shipments stateset-suppliers stateset-invoices stateset-warranties stateset-currency stateset-tax"

    case "\${words[0]}" in
        stateset|ss)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --apply --agent --profile --model --provider --think --stream --budget --memory --no-memory --x402 --resume --json --format --output --verbose --stats --yes --quiet --stdin --batch --parallel --help --version" -- "\${cur}") )
            fi
            ;;
        stateset-chat)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --apply --model --provider --think --stream --budget --memory --no-memory --x402 --verbose --yes --help" -- "\${cur}") )
            fi
            ;;
        stateset-direct)
            if [[ \${cword} -eq 1 ]]; then
                # Resources
                COMPREPLY=( $(compgen -W "customers orders products inventory returns vector c o p i r v cust ord prod inv ret vec stock" -- "\${cur}") )
            elif [[ \${cword} -eq 2 ]]; then
                # Actions based on resource
                case "\${prev}" in
                    customers|c|cust)
                        COMPREPLY=( $(compgen -W "list get create count search" -- "\${cur}") )
                        ;;
                    orders|o|ord)
                        COMPREPLY=( $(compgen -W "list get ship cancel count status pending recent" -- "\${cur}") )
                        ;;
                    products|p|prod)
                        COMPREPLY=( $(compgen -W "list get variant variants count search" -- "\${cur}") )
                        ;;
                    inventory|i|inv|stock)
                        COMPREPLY=( $(compgen -W "list stock adjust create low reserve release" -- "\${cur}") )
                        ;;
                    returns|r|ret)
                        COMPREPLY=( $(compgen -W "list get approve reject count pending create stats" -- "\${cur}") )
                        ;;
                    vector|v|vec)
                        COMPREPLY=( $(compgen -W "search index index-all stats clear clear-all" -- "\${cur}") )
                        ;;
                esac
            elif [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --apply --json --format --output --yes --help" -- "\${cur}") )
            fi
            ;;
        stateset-doctor)
            COMPREPLY=( $(compgen -W "--db --verbose --json --output --checks --fix --help" -- "\${cur}") )
            ;;
        stateset-config)
            if [[ \${cword} -eq 1 ]]; then
                COMPREPLY=( $(compgen -W "set-key show-keys list show create use set get path" -- "\${cur}") )
            elif [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--profile --json --output --help" -- "\${cur}") )
            fi
            ;;
        stateset-sync)
            if [[ \${cword} -eq 1 ]]; then
                COMPREPLY=( $(compgen -W "init status push pull verify conflicts resolve rebase history keys:generate keys:list keys:register keys:rotate keys:export keys:policy keys:expiry keys:batch-rotate groups:create groups:list groups:show groups:add-member groups:remove-member groups:delete groups:refresh-key groups:my-groups" -- "\${cur}") )
            elif [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --json --output --help" -- "\${cur}") )
            fi
            ;;
        stateset-events)
            if [[ \${cword} -eq 1 ]]; then
                COMPREPLY=( $(compgen -W "webhooks" -- "\${cur}") )
            elif [[ \${cword} -eq 2 && "\${prev}" == "webhooks" ]]; then
                COMPREPLY=( $(compgen -W "list add remove test" -- "\${cur}") )
            elif [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --filter --json --output --quiet --help" -- "\${cur}") )
            fi
            ;;
        stateset-channels)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--config --verbose --json --output --help" -- "\${cur}") )
            fi
            ;;
        stateset-daemon)
            if [[ \${cword} -eq 1 ]]; then
                COMPREPLY=( $(compgen -W "install start stop restart enable disable status logs config validate update tailscale ssh-tunnel health uninstall" -- "\${cur}") )
            elif [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--config --port --user --follow --json --output --reverse --persistent --name --help" -- "\${cur}") )
            fi
            ;;
        stateset-skills)
            if [[ \${cword} -eq 1 ]]; then
                COMPREPLY=( $(compgen -W "list search install uninstall info categories marketplace doctor" -- "\${cur}") )
            elif [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--json --output --category --origin --force --help --version" -- "\${cur}") )
            fi
            ;;
        stateset-x402)
            if [[ \${cword} -eq 1 ]]; then
                COMPREPLY=( $(compgen -W "init" -- "\${cur}") )
            elif [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--sequencer-url --tenant-id --store-id --agent-id --network --payer-address --config-dir --config-file --agent-key-id --budget-per-call --budget-daily --starting-balance --max-amount --api-key --jwt --json --output --force --help --version" -- "\${cur}") )
            fi
            ;;
        stateset-x402-mcp)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--config-dir --help --version" -- "\${cur}") )
            fi
            ;;
        stateset-install-service)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--dry-run --uninstall --json --output --help" -- "\${cur}") )
            fi
            ;;
        stateset-pay)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--to --amount --chain --token --agent --order --customer --memo --wallet --balance --chains --apply --json --output --yes --help --version" -- "\${cur}") )
            fi
            ;;
        stateset-autonomous)
            if [[ \${cword} -eq 1 ]]; then
                COMPREPLY=( $(compgen -W "start status init jobs" -- "\${cur}") )
            elif [[ \${cword} -eq 2 && "\${words[1]}" == "jobs" ]]; then
                COMPREPLY=( $(compgen -W "list enable disable run" -- "\${cur}") )
            elif [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --store --port --no-webhooks --no-scheduler --no-workflows --no-policies --no-approvals --init-defaults --notify-config --force --status --enabled --disabled --json --output --verbose --help" -- "\${cur}") )
            fi
            ;;
        stateset-slack)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --apply --model --max-turns --agent --allow --verbose --help" -- "\${cur}") )
            fi
            ;;
        stateset-discord)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --apply --model --max-turns --agent --allow --mention-only --verbose --help" -- "\${cur}") )
            fi
            ;;
        stateset-telegram)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --apply --model --max-turns --agent --allow --verbose --help" -- "\${cur}") )
            fi
            ;;
        stateset-whatsapp)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --apply --model --max-turns --agent --allow --groups --auth-dir --reset --verbose --help" -- "\${cur}") )
            fi
            ;;
        stateset-signal)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --apply --model --max-turns --agent --allow --phone --socket --verbose --help" -- "\${cur}") )
            fi
            ;;
        stateset-google-chat)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --apply --model --max-turns --agent --allow --subscription --verbose --help" -- "\${cur}") )
            fi
            ;;
        stateset-create)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--dir --apply --model --verbose --stats --json --help" -- "\${cur}") )
            fi
            ;;
        stateset-analytics|stateset-checkout|stateset-currency|stateset-inventory|stateset-invoices|stateset-manufacturing|stateset-orders|stateset-payments|stateset-promotions|stateset-returns|stateset-shipments|stateset-subscriptions|stateset-suppliers|stateset-tax|stateset-warranties)
            if [[ "\${cur}" == -* ]]; then
                COMPREPLY=( $(compgen -W "--db --apply --model --provider --think --stream --budget --memory --no-memory --x402 --resume --json --format --output --yes --help --version" -- "\${cur}") )
            fi
            ;;
    esac
}

complete -F _stateset_complete stateset
complete -F _stateset_complete ss
complete -F _stateset_complete stateset-direct
complete -F _stateset_complete stateset-chat
complete -F _stateset_complete stateset-doctor
complete -F _stateset_complete stateset-config
complete -F _stateset_complete stateset-sync
complete -F _stateset_complete stateset-checkout
complete -F _stateset_complete stateset-orders
complete -F _stateset_complete stateset-inventory
complete -F _stateset_complete stateset-returns
complete -F _stateset_complete stateset-analytics
complete -F _stateset_complete stateset-promotions
complete -F _stateset_complete stateset-subscriptions
complete -F _stateset_complete stateset-create
complete -F _stateset_complete stateset-events
complete -F _stateset_complete stateset-channels
complete -F _stateset_complete stateset-daemon
complete -F _stateset_complete stateset-skills
complete -F _stateset_complete stateset-x402
complete -F _stateset_complete stateset-x402-mcp
complete -F _stateset_complete stateset-install-service
complete -F _stateset_complete stateset-pay
complete -F _stateset_complete stateset-autonomous
complete -F _stateset_complete stateset-slack
complete -F _stateset_complete stateset-discord
complete -F _stateset_complete stateset-telegram
complete -F _stateset_complete stateset-whatsapp
complete -F _stateset_complete stateset-signal
complete -F _stateset_complete stateset-google-chat
complete -F _stateset_complete stateset-manufacturing
complete -F _stateset_complete stateset-payments
complete -F _stateset_complete stateset-shipments
complete -F _stateset_complete stateset-suppliers
complete -F _stateset_complete stateset-invoices
complete -F _stateset_complete stateset-warranties
complete -F _stateset_complete stateset-currency
complete -F _stateset_complete stateset-tax
`;
}

function generateZshCompletion() {
  return `#compdef stateset ss stateset-direct stateset-chat stateset-doctor stateset-config stateset-sync stateset-events stateset-channels stateset-daemon stateset-skills stateset-x402 stateset-x402-mcp stateset-install-service stateset-pay stateset-autonomous stateset-slack stateset-discord stateset-telegram stateset-whatsapp stateset-signal stateset-google-chat stateset-create stateset-analytics stateset-checkout stateset-currency stateset-inventory stateset-invoices stateset-manufacturing stateset-orders stateset-payments stateset-promotions stateset-returns stateset-shipments stateset-subscriptions stateset-suppliers stateset-tax stateset-warranties

# StateSet CLI Zsh Completion
# Generated by stateset-completion

_stateset() {
    local curcontext="$curcontext" state line
    typeset -A opt_args

    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--agent[Use specific agent]:agent:' \\
        '--profile[Use profile]:profile:' \\
        '--model[Claude model]:model:(claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022)' \\
        '--provider[AI provider]:provider:(claude openai gemini ollama)' \\
        '--think[Extended thinking]:level:(off low medium high)' \\
        '--stream[Stream output]' \\
        '--budget[Maximum spend in USD]:usd:' \\
        '--memory[Enable memory]' \\
        '--no-memory[Disable memory]' \\
        '--x402[Enable x402 MCP tools]' \\
        '--resume[Resume session]:session_id:' \\
        '--json[JSON output]' \\
        '--format[Output format]:format:(table json csv yaml)' \\
        '--output[Write output to file]:file:_files' \\
        '--verbose[Verbose output]' \\
        '--stats[Show execution statistics]' \\
        '--yes[Skip confirmation prompts]' \\
        '--quiet[Quiet output]' \\
        '--stdin[Read requests from stdin]' \\
        '--batch[Read requests from file]:file:_files' \\
        '--parallel[Parallel requests]:count:' \\
        '--help[Show help]' \\
        '--version[Show version]' \\
        '*::arg:->args'
}

_stateset_chat() {
    local curcontext="$curcontext" state line
    typeset -A opt_args

    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--model[Claude model]:model:(claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022)' \\
        '--provider[AI provider]:provider:(claude openai gemini ollama)' \\
        '--think[Extended thinking]:level:(off low medium high)' \\
        '--stream[Stream output]' \\
        '--budget[Maximum spend in USD]:usd:' \\
        '--memory[Enable memory]' \\
        '--no-memory[Disable memory]' \\
        '--x402[Enable x402 MCP tools]' \\
        '--verbose[Verbose output]' \\
        '--yes[Skip confirmation prompts]' \\
        '--help[Show help]' \\
        '*::arg:->args'
}

_stateset_direct() {
    local curcontext="$curcontext" state line
    typeset -A opt_args

    local resources=(
        'customers:Customer management'
        'orders:Order management'
        'products:Product catalog'
        'inventory:Inventory management'
        'returns:Return processing'
        'vector:Vector search'
        'c:Customers (alias)'
        'o:Orders (alias)'
        'p:Products (alias)'
        'i:Inventory (alias)'
        'r:Returns (alias)'
        'v:Vector (alias)'
        'vec:Vector (alias)'
    )

    local customer_actions=(list get create count search)
    local order_actions=(list get ship cancel count status pending recent)
    local product_actions=(list get variant variants count search)
    local inventory_actions=(list stock adjust create low reserve release)
    local return_actions=(list get approve reject count pending create stats)
    local vector_actions=(search index index-all stats clear clear-all)

    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--json[JSON output]' \\
        '--format[Output format]:format:(table json)' \\
        '--output[Write output to file]:file:_files' \\
        '--yes[Skip confirmation prompts]' \\
        '--help[Show help]' \\
        '1:resource:->resource' \\
        '2:action:->action' \\
        '*::args:->args'

    case "$state" in
        resource)
            _describe -t resources 'resource' resources
            ;;
        action)
            case "$line[1]" in
                customers|c|cust)
                    _describe -t actions 'action' customer_actions
                    ;;
                orders|o|ord)
                    _describe -t actions 'action' order_actions
                    ;;
                products|p|prod)
                    _describe -t actions 'action' product_actions
                    ;;
                inventory|i|inv|stock)
                    _describe -t actions 'action' inventory_actions
                    ;;
                returns|r|ret)
                    _describe -t actions 'action' return_actions
                    ;;
                vector|v|vec)
                    _describe -t actions 'action' vector_actions
                    ;;
            esac
            ;;
    esac
}

_stateset_doctor() {
    _arguments \\
        '--db[Database path]:path:_files' \\
        '--verbose[Verbose output]' \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--checks[Specific checks]:checks:(api db node permissions dependencies sync plugins config system disk)' \\
        '--fix[Attempt to fix issues]' \\
        '--help[Show help]'
}

_stateset_config() {
    local subcommands=(
        'set-key:Set API key'
        'show-keys:Show API keys'
        'list:List profiles'
        'show:Show profile'
        'create:Create profile'
        'use:Switch profile'
        'set:Set config value'
        'get:Get config value'
        'path:Show config path'
    )

    _arguments -C \\
        '--profile[Target profile]:profile:' \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--help[Show help]' \\
        '1:subcommand:->subcommand' \\
        '*::args:->args'

    case "$state" in
        subcommand)
            _describe -t subcommands 'subcommand' subcommands
            ;;
        args)
            if [[ "${words[2]}" == "jobs" ]]; then
                local job_subcommands=(
                    'list:List scheduled jobs'
                    'enable:Enable a job'
                    'disable:Disable a job'
                    'run:Run a job immediately'
                )
                _describe -t job_subcommands 'jobs subcommand' job_subcommands
            fi
            ;;
    esac
}

_stateset_sync() {
    local subcommands=(
        'init:Initialize sync configuration'
        'status:Show sync status'
        'push:Push local changes'
        'pull:Pull remote changes'
        'verify:Verify event inclusion'
        'conflicts:Show conflicts'
        'resolve:Resolve conflict'
        'rebase:Resolve conflicts with strategy'
        'history:Show sync history'
        'keys:generate:Generate keys'
        'keys:list:List keys'
        'keys:register:Register signing key'
        'keys:rotate:Rotate keys'
        'keys:export:Export keys'
        'keys:policy:Manage rotation policy'
        'keys:expiry:Check key expiry'
        'keys:batch-rotate:Batch rotate keys'
        'groups:create:Create group'
        'groups:list:List groups'
        'groups:show:Show group'
        'groups:add-member:Add group member'
        'groups:remove-member:Remove group member'
        'groups:delete:Delete group'
        'groups:refresh-key:Refresh encryption key'
        'groups:my-groups:List your groups'
    )

    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--help[Show help]' \\
        '1:subcommand:->subcommand' \\
        '*::args:->args'

    case "$state" in
        subcommand)
            _describe -t subcommands 'subcommand' subcommands
            ;;
    esac
}

_stateset_agent() {
    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--model[Claude model]:model:(claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022)' \\
        '--provider[AI provider]:provider:(claude openai gemini ollama)' \\
        '--think[Extended thinking]:level:(off low medium high)' \\
        '--stream[Stream output]' \\
        '--budget[Maximum spend in USD]:usd:' \\
        '--memory[Enable memory]' \\
        '--no-memory[Disable memory]' \\
        '--x402[Enable x402 MCP tools]' \\
        '--resume[Resume session]:session_id:' \\
        '--json[JSON output]' \\
        '--format[Output format]:format:(table json csv yaml)' \\
        '--output[Write output to file]:file:_files' \\
        '--yes[Skip confirmation prompts]' \\
        '--help[Show help]' \\
        '--version[Show version]' \\
        '*::arg:->args'
}

_stateset_create() {
    _arguments -C \\
        '--dir[Output directory]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--model[Claude model]:model:(claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022)' \\
        '--verbose[Verbose output]' \\
        '--stats[Show execution statistics]' \\
        '--json[JSON output]' \\
        '--help[Show help]' \\
        '*::arg:->args'
}

_stateset_events() {
    local subcommands=(
        'webhooks:Manage webhooks'
    )

    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--filter[Filter events]:type:(orders inventory customers products returns)' \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--quiet[Quiet output]' \\
        '--help[Show help]' \\
        '1:subcommand:->subcommand' \\
        '*::args:->args'

    case "$state" in
        subcommand)
            _describe -t subcommands 'subcommand' subcommands
            ;;
    esac
}

_stateset_channels() {
    _arguments -C \\
        '--config[Config path]:path:_files' \\
        '--verbose[Verbose output]' \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--help[Show help]' \\
        '*::arg:->args'
}

_stateset_daemon() {
    local subcommands=(
        'install:Install service'
        'start:Start service'
        'stop:Stop service'
        'restart:Restart service'
        'enable:Enable on boot'
        'disable:Disable on boot'
        'status:Show status'
        'logs:Show logs'
        'config:Show config'
        'validate:Validate config'
        'update:Update to latest'
        'tailscale:Tailscale management'
        'ssh-tunnel:SSH tunnel management'
        'health:Health check'
        'uninstall:Remove service'
    )

    _arguments -C \\
        '--config[Config path]:path:_files' \\
        '--port[HTTP port]:port:' \\
        '--user[User mode]' \\
        '--follow[Follow logs]' \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--reverse[Reverse SSH tunnel]' \\
        '--persistent[Persistent SSH tunnel]' \\
        '--name[Tunnel name]:name:' \\
        '--help[Show help]' \\
        '1:subcommand:->subcommand' \\
        '*::args:->args'

    case "$state" in
        subcommand)
            _describe -t subcommands 'subcommand' subcommands
            ;;
    esac
}

_stateset_skills() {
    local subcommands=(
        'list:List skills'
        'search:Search skills'
        'install:Install skill'
        'uninstall:Remove skill'
        'info:Show skill info'
        'categories:List categories'
        'marketplace:Marketplace overview'
        'doctor:Check skill health'
    )

    _arguments -C \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--category[Filter category]:category:' \\
        '--origin[Filter origin]:origin:(bundled installed workspace)' \\
        '--force[Overwrite on install]' \\
        '--help[Show help]' \\
        '--version[Show version]' \\
        '1:subcommand:->subcommand' \\
        '*::args:->args'

    case "$state" in
        subcommand)
            _describe -t subcommands 'subcommand' subcommands
            ;;
    esac
}

_stateset_x402() {
    local subcommands=(
        'init:Initialize x402 configuration'
    )

    _arguments -C \\
        '--sequencer-url[Sequencer URL]:url:' \\
        '--tenant-id[Tenant UUID]:uuid:' \\
        '--store-id[Store UUID]:uuid:' \\
        '--agent-id[Agent ID]:id:' \\
        '--network[Preferred network]:network:' \\
        '--payer-address[Payer wallet address]:address:' \\
        '--config-dir[Config directory]:path:_files' \\
        '--config-file[Config file path]:path:_files' \\
        '--agent-key-id[Signing key ID]:id:' \\
        '--budget-per-call[Max per call]:amount:' \\
        '--budget-daily[Daily budget]:amount:' \\
        '--starting-balance[Starting balance]:amount:' \\
        '--max-amount[Max amount]:amount:' \\
        '--api-key[Sequencer API key]:key:' \\
        '--jwt[Sequencer JWT]:token:' \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--force[Overwrite existing config]' \\
        '--help[Show help]' \\
        '--version[Show version]' \\
        '1:subcommand:->subcommand' \\
        '*::args:->args'

    case "$state" in
        subcommand)
            _describe -t subcommands 'subcommand' subcommands
            ;;
    esac
}

_stateset_x402_mcp() {
    _arguments -C \\
        '--config-dir[Config directory]:path:_files' \\
        '--help[Show help]' \\
        '--version[Show version]' \\
        '*::arg:->args'
}

_stateset_install_service() {
    _arguments -C \\
        '--dry-run[Preview actions without changes]' \\
        '--uninstall[Remove service]' \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--help[Show help]' \\
        '*::arg:->args'
}

_stateset_pay() {
    _arguments -C \\
        '--to[Recipient address]:address:' \\
        '--amount[Amount to send]:amount:' \\
        '--chain[Blockchain network]:chain:' \\
        '--token[Token symbol]:token:' \\
        '--agent[Agent ID]:id:' \\
        '--order[Order ID]:id:' \\
        '--customer[Customer ID]:id:' \\
        '--memo[Payment memo]:text:' \\
        '--wallet[Show wallet address]' \\
        '--balance[Check balance]' \\
        '--chains[List supported chains]' \\
        '--apply[Execute payment]' \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--yes[Skip confirmation prompts]' \\
        '--help[Show help]' \\
        '--version[Show version]' \\
        '*::arg:->args'
}

_stateset_autonomous() {
    local subcommands=(
        'start:Start autonomous engine'
        'status:Show engine status'
        'init:Initialize templates'
        'jobs:Manage scheduled jobs'
    )

    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--store[Engine data path]:path:_files' \\
        '--port[Webhook server port]:port:' \\
        '--no-webhooks[Disable webhooks]' \\
        '--no-scheduler[Disable scheduler]' \\
        '--no-workflows[Disable workflows]' \\
        '--no-policies[Disable policies]' \\
        '--no-approvals[Disable approvals]' \\
        '--init-defaults[Initialize defaults]' \\
        '--notify-config[Notification config]:path:_files' \\
        '--force[Overwrite existing autonomous data]' \\
        '--status[Filter jobs by status]:status:(pending running completed failed paused cancelled)' \\
        '--enabled[Only enabled jobs]' \\
        '--disabled[Only disabled jobs]' \\
        '--json[JSON output]' \\
        '--output[Write output to file]:file:_files' \\
        '--enable[Enable job]:id:' \\
        '--disable[Disable job]:id:' \\
        '--run[Run job now]:id:' \\
        '--verbose[Verbose output]' \\
        '--help[Show help]' \\
        '1:subcommand:->subcommand' \\
        '*::args:->args'

    case "$state" in
        subcommand)
            _describe -t subcommands 'subcommand' subcommands
            ;;
    esac
}

_stateset_slack() {
    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--model[Claude model]:model:(claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022)' \\
        '--max-turns[Max turns per message]:count:' \\
        '--agent[Force agent]:agent:' \\
        '--allow[Allowlist user IDs]:ids:' \\
        '--verbose[Verbose output]' \\
        '--help[Show help]' \\
        '*::arg:->args'
}

_stateset_discord() {
    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--model[Claude model]:model:(claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022)' \\
        '--max-turns[Max turns per message]:count:' \\
        '--agent[Force agent]:agent:' \\
        '--allow[Allowlist user IDs]:ids:' \\
        '--mention-only[Respond only when mentioned]' \\
        '--verbose[Verbose output]' \\
        '--help[Show help]' \\
        '*::arg:->args'
}

_stateset_telegram() {
    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--model[Claude model]:model:(claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022)' \\
        '--max-turns[Max turns per message]:count:' \\
        '--agent[Force agent]:agent:' \\
        '--allow[Allowlist user IDs]:ids:' \\
        '--verbose[Verbose output]' \\
        '--help[Show help]' \\
        '*::arg:->args'
}

_stateset_whatsapp() {
    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--model[Claude model]:model:(claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022)' \\
        '--max-turns[Max turns per message]:count:' \\
        '--agent[Force agent]:agent:' \\
        '--allow[Allowlist phone numbers]:phones:' \\
        '--groups[Respond in group chats]' \\
        '--auth-dir[Auth directory]:path:_files' \\
        '--reset[Reset auth]' \\
        '--verbose[Verbose output]' \\
        '--help[Show help]' \\
        '*::arg:->args'
}

_stateset_signal() {
    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--model[Claude model]:model:(claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022)' \\
        '--max-turns[Max turns per message]:count:' \\
        '--agent[Force agent]:agent:' \\
        '--allow[Allowlist phone numbers]:phones:' \\
        '--phone[Registered Signal phone]:phone:' \\
        '--socket[Signal daemon socket]:path:_files' \\
        '--verbose[Verbose output]' \\
        '--help[Show help]' \\
        '*::arg:->args'
}

_stateset_google_chat() {
    _arguments -C \\
        '--db[Database path]:path:_files' \\
        '--apply[Enable write operations]' \\
        '--model[Claude model]:model:(claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022)' \\
        '--max-turns[Max turns per message]:count:' \\
        '--agent[Force agent]:agent:' \\
        '--allow[Allowlist user IDs]:ids:' \\
        '--subscription[Pub/Sub subscription]:name:' \\
        '--verbose[Verbose output]' \\
        '--help[Show help]' \\
        '*::arg:->args'
}

compdef _stateset stateset
compdef _stateset ss
compdef _stateset_chat stateset-chat
compdef _stateset_direct stateset-direct
compdef _stateset_doctor stateset-doctor
compdef _stateset_config stateset-config
compdef _stateset_sync stateset-sync
compdef _stateset_events stateset-events
compdef _stateset_channels stateset-channels
compdef _stateset_daemon stateset-daemon
compdef _stateset_skills stateset-skills
compdef _stateset_x402 stateset-x402
compdef _stateset_x402_mcp stateset-x402-mcp
compdef _stateset_install_service stateset-install-service
compdef _stateset_pay stateset-pay
compdef _stateset_autonomous stateset-autonomous
compdef _stateset_slack stateset-slack
compdef _stateset_discord stateset-discord
compdef _stateset_telegram stateset-telegram
compdef _stateset_whatsapp stateset-whatsapp
compdef _stateset_signal stateset-signal
compdef _stateset_google_chat stateset-google-chat
compdef _stateset_create stateset-create
compdef _stateset_agent stateset-analytics
compdef _stateset_agent stateset-checkout
compdef _stateset_agent stateset-currency
compdef _stateset_agent stateset-inventory
compdef _stateset_agent stateset-invoices
compdef _stateset_agent stateset-manufacturing
compdef _stateset_agent stateset-orders
compdef _stateset_agent stateset-payments
compdef _stateset_agent stateset-promotions
compdef _stateset_agent stateset-returns
compdef _stateset_agent stateset-shipments
compdef _stateset_agent stateset-subscriptions
compdef _stateset_agent stateset-suppliers
compdef _stateset_agent stateset-tax
compdef _stateset_agent stateset-warranties
`;
}

function generateFishCompletion() {
  return `# StateSet CLI Fish Completion
# Generated by stateset-completion

# Disable file completion by default
complete -c stateset -f
complete -c ss -w stateset
complete -c stateset-direct -f
complete -c stateset-chat -f
complete -c stateset-doctor -f
complete -c stateset-config -f
complete -c stateset-sync -f
complete -c stateset-events -f
complete -c stateset-channels -f
complete -c stateset-daemon -f
complete -c stateset-skills -f
complete -c stateset-create -f
complete -c stateset-x402 -f
complete -c stateset-x402-mcp -f
complete -c stateset-install-service -f
complete -c stateset-pay -f
complete -c stateset-autonomous -f
complete -c stateset-slack -f
complete -c stateset-discord -f
complete -c stateset-telegram -f
complete -c stateset-whatsapp -f
complete -c stateset-signal -f
complete -c stateset-google-chat -f

# stateset main command
complete -c stateset -l db -d 'Database path' -r
complete -c stateset -l apply -d 'Enable write operations'
complete -c stateset -l agent -d 'Use specific agent' -r
complete -c stateset -l profile -s p -d 'Use profile' -r
complete -c stateset -l model -d 'Claude model' -xa 'claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022'
complete -c stateset -l provider -d 'AI provider' -xa 'claude openai gemini ollama'
complete -c stateset -l think -d 'Extended thinking' -xa 'off low medium high'
complete -c stateset -l stream -d 'Stream output'
complete -c stateset -l budget -d 'Maximum spend in USD' -r
complete -c stateset -l memory -d 'Enable memory'
complete -c stateset -l no-memory -d 'Disable memory'
complete -c stateset -l x402 -d 'Enable x402 MCP tools'
complete -c stateset -l resume -d 'Resume session' -r
complete -c stateset -l json -d 'JSON output'
complete -c stateset -l format -d 'Output format' -xa 'table json csv yaml'
complete -c stateset -l output -d 'Write output to file' -r
complete -c stateset -l verbose -s V -d 'Verbose output'
complete -c stateset -l stats -d 'Show execution statistics'
complete -c stateset -l yes -s y -d 'Skip confirmation prompts'
complete -c stateset -l quiet -s q -d 'Quiet output'
complete -c stateset -l stdin -d 'Read requests from stdin'
complete -c stateset -l batch -d 'Read requests from file' -r
complete -c stateset -l parallel -d 'Parallel requests' -r
complete -c stateset -l help -d 'Show help'
complete -c stateset -l version -d 'Show version'

# stateset-chat
complete -c stateset-chat -l db -d 'Database path' -r
complete -c stateset-chat -l apply -d 'Enable write operations'
complete -c stateset-chat -l model -d 'Claude model' -xa 'claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022'
complete -c stateset-chat -l provider -d 'AI provider' -xa 'claude openai gemini ollama'
complete -c stateset-chat -l think -d 'Extended thinking' -xa 'off low medium high'
complete -c stateset-chat -l stream -d 'Stream output'
complete -c stateset-chat -l budget -d 'Maximum spend in USD' -r
complete -c stateset-chat -l memory -d 'Enable memory'
complete -c stateset-chat -l no-memory -d 'Disable memory'
complete -c stateset-chat -l x402 -d 'Enable x402 MCP tools'
complete -c stateset-chat -l verbose -s V -d 'Verbose output'
complete -c stateset-chat -l yes -s y -d 'Skip confirmation prompts'
complete -c stateset-chat -l help -d 'Show help'

# stateset-direct resources
complete -c stateset-direct -n '__fish_is_first_arg' -a 'customers' -d 'Customer management'
complete -c stateset-direct -n '__fish_is_first_arg' -a 'orders' -d 'Order management'
complete -c stateset-direct -n '__fish_is_first_arg' -a 'products' -d 'Product catalog'
complete -c stateset-direct -n '__fish_is_first_arg' -a 'inventory' -d 'Inventory management'
complete -c stateset-direct -n '__fish_is_first_arg' -a 'returns' -d 'Return processing'
complete -c stateset-direct -n '__fish_is_first_arg' -a 'vector' -d 'Vector search'
complete -c stateset-direct -n '__fish_is_first_arg' -a 'c o p i r v vec' -d 'Aliases'

# Customer actions
complete -c stateset-direct -n '__fish_seen_argument customers c cust' -a 'list' -d 'List customers'
complete -c stateset-direct -n '__fish_seen_argument customers c cust' -a 'get' -d 'Get customer'
complete -c stateset-direct -n '__fish_seen_argument customers c cust' -a 'create' -d 'Create customer'
complete -c stateset-direct -n '__fish_seen_argument customers c cust' -a 'count' -d 'Count customers'
complete -c stateset-direct -n '__fish_seen_argument customers c cust' -a 'search' -d 'Search customers'

# Order actions
complete -c stateset-direct -n '__fish_seen_argument orders o ord' -a 'list' -d 'List orders'
complete -c stateset-direct -n '__fish_seen_argument orders o ord' -a 'get' -d 'Get order'
complete -c stateset-direct -n '__fish_seen_argument orders o ord' -a 'ship' -d 'Ship order'
complete -c stateset-direct -n '__fish_seen_argument orders o ord' -a 'cancel' -d 'Cancel order'
complete -c stateset-direct -n '__fish_seen_argument orders o ord' -a 'count' -d 'Count orders'
complete -c stateset-direct -n '__fish_seen_argument orders o ord' -a 'status' -d 'Update status'
complete -c stateset-direct -n '__fish_seen_argument orders o ord' -a 'pending' -d 'Pending orders'
complete -c stateset-direct -n '__fish_seen_argument orders o ord' -a 'recent' -d 'Recent orders'

# Product actions
complete -c stateset-direct -n '__fish_seen_argument products p prod' -a 'list' -d 'List products'
complete -c stateset-direct -n '__fish_seen_argument products p prod' -a 'get' -d 'Get product'
complete -c stateset-direct -n '__fish_seen_argument products p prod' -a 'variant' -d 'Get variant by SKU'
complete -c stateset-direct -n '__fish_seen_argument products p prod' -a 'variants' -d 'List variants'
complete -c stateset-direct -n '__fish_seen_argument products p prod' -a 'count' -d 'Count products'
complete -c stateset-direct -n '__fish_seen_argument products p prod' -a 'search' -d 'Search products'

# Inventory actions
complete -c stateset-direct -n '__fish_seen_argument inventory i inv stock' -a 'list' -d 'List inventory'
complete -c stateset-direct -n '__fish_seen_argument inventory i inv stock' -a 'stock' -d 'Get stock level'
complete -c stateset-direct -n '__fish_seen_argument inventory i inv stock' -a 'adjust' -d 'Adjust stock'
complete -c stateset-direct -n '__fish_seen_argument inventory i inv stock' -a 'create' -d 'Create item'
complete -c stateset-direct -n '__fish_seen_argument inventory i inv stock' -a 'low' -d 'Low stock items'
complete -c stateset-direct -n '__fish_seen_argument inventory i inv stock' -a 'reserve' -d 'Reserve inventory'
complete -c stateset-direct -n '__fish_seen_argument inventory i inv stock' -a 'release' -d 'Release reservation'

# Return actions
complete -c stateset-direct -n '__fish_seen_argument returns r ret' -a 'list' -d 'List returns'
complete -c stateset-direct -n '__fish_seen_argument returns r ret' -a 'get' -d 'Get return'
complete -c stateset-direct -n '__fish_seen_argument returns r ret' -a 'approve' -d 'Approve return'
complete -c stateset-direct -n '__fish_seen_argument returns r ret' -a 'reject' -d 'Reject return'
complete -c stateset-direct -n '__fish_seen_argument returns r ret' -a 'count' -d 'Count returns'
complete -c stateset-direct -n '__fish_seen_argument returns r ret' -a 'pending' -d 'Pending returns'
complete -c stateset-direct -n '__fish_seen_argument returns r ret' -a 'create' -d 'Create return'
complete -c stateset-direct -n '__fish_seen_argument returns r ret' -a 'stats' -d 'Return statistics'

# Vector actions
complete -c stateset-direct -n '__fish_seen_argument vector v vec' -a 'search' -d 'Vector search'
complete -c stateset-direct -n '__fish_seen_argument vector v vec' -a 'index' -d 'Index entity'
complete -c stateset-direct -n '__fish_seen_argument vector v vec' -a 'index-all' -d 'Index all entities'
complete -c stateset-direct -n '__fish_seen_argument vector v vec' -a 'stats' -d 'Embedding stats'
complete -c stateset-direct -n '__fish_seen_argument vector v vec' -a 'clear' -d 'Clear embeddings'
complete -c stateset-direct -n '__fish_seen_argument vector v vec' -a 'clear-all' -d 'Clear all embeddings'

# Options
complete -c stateset-direct -l db -d 'Database path' -r
complete -c stateset-direct -l apply -d 'Enable write operations'
complete -c stateset-direct -l json -d 'JSON output'
complete -c stateset-direct -l format -d 'Output format' -xa 'table json'
complete -c stateset-direct -l output -d 'Write output to file' -r
complete -c stateset-direct -l yes -s y -d 'Skip confirmation prompts'
complete -c stateset-direct -l help -d 'Show help'

# stateset-doctor
complete -c stateset-doctor -l db -d 'Database path' -r
complete -c stateset-doctor -l verbose -s V -d 'Verbose output'
complete -c stateset-doctor -l json -d 'JSON output'
complete -c stateset-doctor -l output -d 'Write output to file' -r
complete -c stateset-doctor -l checks -d 'Specific checks' -xa 'api db node permissions dependencies sync plugins config system disk'
complete -c stateset-doctor -l fix -d 'Attempt to fix issues'
complete -c stateset-doctor -l help -d 'Show help'

# stateset-config
complete -c stateset-config -n '__fish_is_first_arg' -a 'set-key' -d 'Set API key'
complete -c stateset-config -n '__fish_is_first_arg' -a 'show-keys' -d 'Show API keys'
complete -c stateset-config -n '__fish_is_first_arg' -a 'list' -d 'List profiles'
complete -c stateset-config -n '__fish_is_first_arg' -a 'show' -d 'Show profile'
complete -c stateset-config -n '__fish_is_first_arg' -a 'create' -d 'Create profile'
complete -c stateset-config -n '__fish_is_first_arg' -a 'use' -d 'Switch profile'
complete -c stateset-config -n '__fish_is_first_arg' -a 'set' -d 'Set config value'
complete -c stateset-config -n '__fish_is_first_arg' -a 'get' -d 'Get config value'
complete -c stateset-config -n '__fish_is_first_arg' -a 'path' -d 'Show config path'
complete -c stateset-config -l profile -s p -d 'Target profile' -r
complete -c stateset-config -l json -d 'JSON output'
complete -c stateset-config -l output -d 'Write output to file' -r
complete -c stateset-config -l help -d 'Show help'

# stateset-sync
complete -c stateset-sync -n '__fish_is_first_arg' -a 'init' -d 'Initialize sync configuration'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'status' -d 'Show sync status'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'push' -d 'Push local changes'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'pull' -d 'Pull remote changes'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'verify' -d 'Verify event inclusion'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'conflicts' -d 'Show conflicts'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'resolve' -d 'Resolve conflict'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'rebase' -d 'Resolve conflicts with strategy'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'history' -d 'Show sync history'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'keys:generate' -d 'Generate keys'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'keys:list' -d 'List keys'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'keys:register' -d 'Register signing key'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'keys:rotate' -d 'Rotate keys'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'keys:export' -d 'Export keys'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'keys:policy' -d 'Manage rotation policy'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'keys:expiry' -d 'Check key expiry'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'keys:batch-rotate' -d 'Batch rotate keys'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'groups:create' -d 'Create group'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'groups:list' -d 'List groups'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'groups:show' -d 'Show group'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'groups:add-member' -d 'Add group member'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'groups:remove-member' -d 'Remove group member'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'groups:delete' -d 'Delete group'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'groups:refresh-key' -d 'Refresh encryption key'
complete -c stateset-sync -n '__fish_is_first_arg' -a 'groups:my-groups' -d 'List your groups'
complete -c stateset-sync -l db -d 'Database path' -r
complete -c stateset-sync -l json -d 'JSON output'
complete -c stateset-sync -l output -d 'Write output to file' -r
complete -c stateset-sync -l help -d 'Show help'

# stateset-events
complete -c stateset-events -n '__fish_is_first_arg' -a 'webhooks' -d 'Manage webhooks'
complete -c stateset-events -l db -d 'Database path' -r
complete -c stateset-events -l filter -d 'Filter events' -xa 'orders inventory customers products returns'
complete -c stateset-events -l json -d 'JSON output'
complete -c stateset-events -l output -d 'Write output to file' -r
complete -c stateset-events -l quiet -s q -d 'Quiet output'
complete -c stateset-events -l help -d 'Show help'

# stateset-channels
complete -c stateset-channels -l config -d 'Config path' -r
complete -c stateset-channels -l verbose -s V -d 'Verbose output'
complete -c stateset-channels -l json -d 'JSON output'
complete -c stateset-channels -l output -d 'Write output to file' -r
complete -c stateset-channels -l help -d 'Show help'

# stateset-daemon
complete -c stateset-daemon -n '__fish_is_first_arg' -a 'install start stop restart enable disable status logs config validate update tailscale ssh-tunnel health uninstall' -d 'Daemon commands'
complete -c stateset-daemon -l config -d 'Config path' -r
complete -c stateset-daemon -l port -d 'HTTP port' -r
complete -c stateset-daemon -l user -d 'User mode'
complete -c stateset-daemon -l follow -s f -d 'Follow logs'
complete -c stateset-daemon -l json -d 'JSON output'
complete -c stateset-daemon -l output -d 'Write output to file' -r
complete -c stateset-daemon -l reverse -d 'Reverse SSH tunnel'
complete -c stateset-daemon -l persistent -d 'Persistent SSH tunnel'
complete -c stateset-daemon -l name -d 'Tunnel name' -r
complete -c stateset-daemon -l help -d 'Show help'

# stateset-skills
complete -c stateset-skills -n '__fish_is_first_arg' -a 'list search install uninstall info categories marketplace doctor' -d 'Skill commands'
complete -c stateset-skills -l json -d 'JSON output'
complete -c stateset-skills -l output -d 'Write output to file' -r
complete -c stateset-skills -l category -s c -d 'Filter by category' -r
complete -c stateset-skills -l origin -s o -d 'Filter by origin' -xa 'bundled installed workspace'
complete -c stateset-skills -l force -d 'Overwrite on install'
complete -c stateset-skills -l help -d 'Show help'
complete -c stateset-skills -l version -d 'Show version'

# stateset-create
complete -c stateset-create -l dir -d 'Output directory' -r
complete -c stateset-create -l apply -d 'Enable write operations'
complete -c stateset-create -l model -d 'Claude model' -xa 'claude-sonnet-4-5-20250929 claude-opus-4-5-20251101 claude-haiku-3-5-20241022'
complete -c stateset-create -l verbose -s V -d 'Verbose output'
complete -c stateset-create -l stats -d 'Show execution statistics'
complete -c stateset-create -l json -d 'JSON output'
complete -c stateset-create -l help -d 'Show help'

# stateset-pay
complete -c stateset-pay -l to -s t -d 'Recipient address' -r
complete -c stateset-pay -l amount -s a -d 'Amount to send' -r
complete -c stateset-pay -l chain -s c -d 'Blockchain network' -r
complete -c stateset-pay -l token -d 'Token symbol' -r
complete -c stateset-pay -l agent -d 'Agent ID' -r
complete -c stateset-pay -l order -d 'Order ID' -r
complete -c stateset-pay -l customer -d 'Customer ID' -r
complete -c stateset-pay -l memo -d 'Payment memo' -r
complete -c stateset-pay -l wallet -s w -d 'Show wallet address'
complete -c stateset-pay -l balance -s b -d 'Check balance'
complete -c stateset-pay -l chains -d 'List supported chains'
complete -c stateset-pay -l apply -d 'Execute payment'
complete -c stateset-pay -l json -d 'JSON output'
complete -c stateset-pay -l output -d 'Write output to file' -r
complete -c stateset-pay -l yes -s y -d 'Skip confirmation prompts'
complete -c stateset-pay -l help -s h -d 'Show help'
complete -c stateset-pay -l version -s v -d 'Show version'

# stateset-autonomous
complete -c stateset-autonomous -n '__fish_is_first_arg' -a 'start status init jobs' -d 'Autonomous commands'
complete -c stateset-autonomous -n '__fish_seen_subcommand_from jobs; and __fish_is_nth_token 2' -a 'list enable disable run' -d 'Jobs commands'
complete -c stateset-autonomous -l db -d 'Database path' -r
complete -c stateset-autonomous -l store -s s -d 'Engine data path' -r
complete -c stateset-autonomous -l port -s p -d 'Webhook server port' -r
complete -c stateset-autonomous -l no-webhooks -d 'Disable webhooks'
complete -c stateset-autonomous -l no-scheduler -d 'Disable scheduler'
complete -c stateset-autonomous -l no-workflows -d 'Disable workflows'
complete -c stateset-autonomous -l no-policies -d 'Disable policies'
complete -c stateset-autonomous -l no-approvals -d 'Disable approvals'
complete -c stateset-autonomous -l init-defaults -d 'Initialize defaults'
complete -c stateset-autonomous -l notify-config -d 'Notification config' -r
complete -c stateset-autonomous -l force -d 'Overwrite existing autonomous data'
complete -c stateset-autonomous -l status -d 'Filter jobs by status' -r
complete -c stateset-autonomous -l enabled -d 'Only enabled jobs'
complete -c stateset-autonomous -l disabled -d 'Only disabled jobs'
complete -c stateset-autonomous -l json -d 'JSON output'
complete -c stateset-autonomous -l output -d 'Write output to file' -r
complete -c stateset-autonomous -l enable -d 'Enable job' -r
complete -c stateset-autonomous -l disable -d 'Disable job' -r
complete -c stateset-autonomous -l run -d 'Run job now' -r
complete -c stateset-autonomous -l verbose -s v -d 'Verbose output'
complete -c stateset-autonomous -l help -d 'Show help'

# stateset-slack
complete -c stateset-slack -l db -d 'Database path' -r
complete -c stateset-slack -l apply -d 'Enable write operations'
complete -c stateset-slack -l model -d 'Claude model' -r
complete -c stateset-slack -l max-turns -d 'Max turns per message' -r
complete -c stateset-slack -l agent -d 'Force agent' -r
complete -c stateset-slack -l allow -d 'Allowlist user IDs' -r
complete -c stateset-slack -l verbose -s V -d 'Verbose output'
complete -c stateset-slack -l help -s h -d 'Show help'

# stateset-discord
complete -c stateset-discord -l db -d 'Database path' -r
complete -c stateset-discord -l apply -d 'Enable write operations'
complete -c stateset-discord -l model -d 'Claude model' -r
complete -c stateset-discord -l max-turns -d 'Max turns per message' -r
complete -c stateset-discord -l agent -d 'Force agent' -r
complete -c stateset-discord -l allow -d 'Allowlist user IDs' -r
complete -c stateset-discord -l mention-only -d 'Respond only when mentioned'
complete -c stateset-discord -l verbose -s V -d 'Verbose output'
complete -c stateset-discord -l help -s h -d 'Show help'

# stateset-telegram
complete -c stateset-telegram -l db -d 'Database path' -r
complete -c stateset-telegram -l apply -d 'Enable write operations'
complete -c stateset-telegram -l model -d 'Claude model' -r
complete -c stateset-telegram -l max-turns -d 'Max turns per message' -r
complete -c stateset-telegram -l agent -d 'Force agent' -r
complete -c stateset-telegram -l allow -d 'Allowlist user IDs' -r
complete -c stateset-telegram -l verbose -s V -d 'Verbose output'
complete -c stateset-telegram -l help -s h -d 'Show help'

# stateset-whatsapp
complete -c stateset-whatsapp -l db -d 'Database path' -r
complete -c stateset-whatsapp -l apply -d 'Enable write operations'
complete -c stateset-whatsapp -l model -d 'Claude model' -r
complete -c stateset-whatsapp -l max-turns -d 'Max turns per message' -r
complete -c stateset-whatsapp -l agent -d 'Force agent' -r
complete -c stateset-whatsapp -l allow -d 'Allowlist phone numbers' -r
complete -c stateset-whatsapp -l groups -d 'Respond in group chats'
complete -c stateset-whatsapp -l auth-dir -d 'Auth directory' -r
complete -c stateset-whatsapp -l reset -d 'Reset auth'
complete -c stateset-whatsapp -l verbose -s V -d 'Verbose output'
complete -c stateset-whatsapp -l help -s h -d 'Show help'

# stateset-signal
complete -c stateset-signal -l db -d 'Database path' -r
complete -c stateset-signal -l apply -d 'Enable write operations'
complete -c stateset-signal -l model -d 'Claude model' -r
complete -c stateset-signal -l max-turns -d 'Max turns per message' -r
complete -c stateset-signal -l agent -d 'Force agent' -r
complete -c stateset-signal -l allow -d 'Allowlist phone numbers' -r
complete -c stateset-signal -l phone -d 'Registered Signal phone' -r
complete -c stateset-signal -l socket -d 'Signal daemon socket' -r
complete -c stateset-signal -l verbose -s V -d 'Verbose output'
complete -c stateset-signal -l help -s h -d 'Show help'

# stateset-google-chat
complete -c stateset-google-chat -l db -d 'Database path' -r
complete -c stateset-google-chat -l apply -d 'Enable write operations'
complete -c stateset-google-chat -l model -d 'Claude model' -r
complete -c stateset-google-chat -l max-turns -d 'Max turns per message' -r
complete -c stateset-google-chat -l agent -d 'Force agent' -r
complete -c stateset-google-chat -l allow -d 'Allowlist user IDs' -r
complete -c stateset-google-chat -l subscription -d 'Pub/Sub subscription' -r
complete -c stateset-google-chat -l verbose -s V -d 'Verbose output'
complete -c stateset-google-chat -l help -s h -d 'Show help'

# stateset-x402
complete -c stateset-x402 -n '__fish_is_first_arg' -a 'init' -d 'Initialize x402 configuration'
complete -c stateset-x402 -l sequencer-url -d 'Sequencer URL' -r
complete -c stateset-x402 -l tenant-id -d 'Tenant UUID' -r
complete -c stateset-x402 -l store-id -d 'Store UUID' -r
complete -c stateset-x402 -l agent-id -d 'Agent ID' -r
complete -c stateset-x402 -l network -d 'Preferred network' -r
complete -c stateset-x402 -l payer-address -d 'Payer wallet address' -r
complete -c stateset-x402 -l config-dir -d 'Config directory' -r
complete -c stateset-x402 -l config-file -d 'Config file path' -r
complete -c stateset-x402 -l agent-key-id -d 'Signing key ID' -r
complete -c stateset-x402 -l budget-per-call -d 'Max per call' -r
complete -c stateset-x402 -l budget-daily -d 'Daily budget' -r
complete -c stateset-x402 -l starting-balance -d 'Starting balance' -r
complete -c stateset-x402 -l max-amount -d 'Max amount' -r
complete -c stateset-x402 -l api-key -d 'Sequencer API key' -r
complete -c stateset-x402 -l jwt -d 'Sequencer JWT' -r
complete -c stateset-x402 -l json -d 'JSON output'
complete -c stateset-x402 -l output -d 'Write output to file' -r
complete -c stateset-x402 -l force -d 'Overwrite existing config'
complete -c stateset-x402 -l help -d 'Show help'
complete -c stateset-x402 -l version -d 'Show version'

# stateset-x402-mcp
complete -c stateset-x402-mcp -l config-dir -d 'Config directory' -r
complete -c stateset-x402-mcp -l help -d 'Show help'
complete -c stateset-x402-mcp -l version -d 'Show version'

# stateset-install-service
complete -c stateset-install-service -l dry-run -d 'Preview actions without changes'
complete -c stateset-install-service -l uninstall -d 'Remove service'
complete -c stateset-install-service -l json -d 'JSON output'
complete -c stateset-install-service -l output -d 'Write output to file' -r
complete -c stateset-install-service -l help -s h -d 'Show help'
`;
}

// Main
const args = process.argv.slice(2);
const shell = args[0];

if (!shell || shell === '--help' || shell === '-h') {
  console.log(HELP);
  process.exit(0);
}

switch (shell.toLowerCase()) {
  case 'bash':
    console.log(generateBashCompletion());
    break;
  case 'zsh':
    console.log(generateZshCompletion());
    break;
  case 'fish':
    console.log(generateFishCompletion());
    break;
  default:
    console.error(`Unknown shell: ${shell}`);
    console.error('Supported shells: bash, zsh, fish');
    process.exit(1);
}
