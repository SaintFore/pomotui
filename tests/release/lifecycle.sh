#!/usr/bin/env bash
# Sourced by acceptance.sh in the disposable guest; uses its real package inputs.
lifecycle_dir="$output/lifecycle"
mkdir -p "$lifecycle_dir"
for state in stopped failed start-limit disabled conflict; do
    echo "LIFECYCLE: $state"
    as_user systemctl --user disable --now pomotui.socket
    as_user systemctl --user stop pomotui.service
    rm -rf "/home/$user/.config/systemd/user/pomotui.service.d"
    rm -f "/home/$user/.config/systemd/user/pomotui.service"
    pacman -U --noconfirm "$old"
    as_user systemctl --user daemon-reload
    as_user systemctl --user reset-failed pomotui.service pomotui.socket
    as_user systemctl --user enable --now pomotui.socket
    as_user pomotui status --json > "$lifecycle_dir/$state-before.json"
    case "$state" in
        stopped) as_user systemctl --user stop pomotui.service ;;
        failed|start-limit)
            # Real process failures, not mocked service-manager flags.
            for attempt in $(seq 1 8); do
                as_user systemctl --user kill --signal=KILL pomotui.service || true
                sleep 0.1
            done
            if [[ "$state" == start-limit ]]; then
                [[ "$(as_user systemctl --user show pomotui.service -p Result --value)" == start-limit-hit ]]
            else
                as_user systemctl --user is-failed pomotui.service
            fi
            ;;
        disabled) as_user systemctl --user disable --now pomotui.socket; as_user systemctl --user stop pomotui.service ;;
        conflict)
            as_user systemctl --user stop pomotui.service
            install -d -o "$user" -g "$user" "/home/$user/.config/systemd/user"
            cp /usr/lib/systemd/user/pomotui.service "/home/$user/.config/systemd/user/pomotui.service"
            sed -i 's|/usr/bin/pomotui-service|/home/pomotui-acceptance/.local/bin/pomotui-service|' "/home/$user/.config/systemd/user/pomotui.service"
            sha256sum "/home/$user/.config/systemd/user/pomotui.service" > "$lifecycle_dir/conflict-preserved.sha256"
            as_user systemctl --user daemon-reload ;;
    esac
    as_user systemctl --user show pomotui.service > "$lifecycle_dir/$state-before-unit.txt"
    pacman -U --noconfirm "$candidate" 2>&1 | tee "$lifecycle_dir/$state-upgrade.log"
    case "$state" in
        disabled)
            ! as_user systemctl --user is-enabled --quiet pomotui.socket
            ! as_user systemctl --user is-active --quiet pomotui.service
            ! as_user systemctl --user is-active --quiet pomotui.socket ;;
        conflict)
            sha256sum --check "$lifecycle_dir/conflict-preserved.sha256"
            grep -F "/home/$user/.config/systemd/user/pomotui.service" "$lifecycle_dir/$state-upgrade.log"
            grep -F 'conflict' "$lifecycle_dir/$state-upgrade.log" ;;
        *)
            as_user systemctl --user is-active pomotui.service
            pid=$(as_user systemctl --user show pomotui.service -p MainPID --value)
            [[ "$(sha256sum "/proc/$pid/exe" | cut -d' ' -f1)" == "$(sha256sum /usr/bin/pomotui-service | cut -d' ' -f1)" ]]
            as_user pomotui status --json > "$lifecycle_dir/$state-after.json" ;;
    esac
    echo "PASS: lifecycle $state"
done
