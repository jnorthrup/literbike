# litebike bash completion
# Install: litebike completion > /usr/local/share/bash-completion/completions/litebike
# Or:      source <(litebike completion)

_litebike_completion() {
    local cur prev
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"

    local commands="ifconfig route netstat ip proxy-quick knox-proxy proxy-config proxy-setup proxy-server proxy-client proxy-node proxy-cleanup watch probe domains carrier radios scan-ports git-push git-sync ssh-deploy remote-sync pattern-match pattern-glob pattern-regex pattern-scan pattern-bench snapshot upnp-gateway bonjour-discover completion carrier-bypass raw-connect trust-host bootstrap quic-vqa"

    COMPREPLY=( $(compgen -W "${commands}" -- "${cur}") )
    return 0
}

complete -F _litebike_completion litebike
