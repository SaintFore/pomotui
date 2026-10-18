#!/usr/bin/env bash
# Sourced by acceptance.sh in the disposable guest; uses its real package inputs.
lifecycle_dir="$output/lifecycle"
mkdir -p "$lifecycle_dir"
for state in stopped failed start-limit disabled conflict dropin; do
    echo "LIFECYCLE: $state"
    as_user systemctl --user disable --now pomotui.socket
    as_user systemctl --user stop pomotui.service
    rm -rf "/home/$user/.config/systemd/user/pomotui.service.d"
    rm -f "/home/$user/.config/systemd/user/pomotui.service"
    rm -rf "/home/$user/.local/share/pomotui" "/home/$user/.config/pomotui"
    pacman -U --noconfirm --ask=4 "$old"
    as_user systemctl --user daemon-reload
    as_user systemctl --user reset-failed
    as_user systemctl --user enable --now pomotui.socket
    as_user pomotui status --json > "$lifecycle_dir/$state-before.json"
    case "$state" in
        stopped) as_user systemctl --user stop pomotui.service ;;
        failed)
            runtime_dropin="/run/user/$uid/systemd/user/pomotui.service.d"
            install -d -o "$user" -g "$user" "$runtime_dropin"
            printf '[Service]\nRestart=no\n' > "$runtime_dropin/failure.conf"
            as_user systemctl --user daemon-reload
            as_user systemctl --user kill --signal=KILL pomotui.service
            sleep 0.2
            as_user systemctl --user is-failed pomotui.service
            rm "$runtime_dropin/failure.conf"
            as_user systemctl --user daemon-reload ;;
        start-limit)
            # Actual crash/restart loop until the real manager refuses starts.
            for attempt in $(seq 1 20); do
                as_user systemctl --user kill --signal=KILL pomotui.service || true
                sleep 0.2
                if as_user systemctl --user is-failed --quiet pomotui.service; then break; fi
            done
            [[ "$(as_user systemctl --user show pomotui.service -p Result --value)" == start-limit-hit ]]
            ;;
        disabled) as_user systemctl --user disable --now pomotui.socket; as_user systemctl --user stop pomotui.service ;;
        conflict|dropin)
            as_user systemctl --user stop pomotui.service
            install -d -o "$user" -g "$user" "/home/$user/.config/systemd/user" "/home/$user/.local/bin"
            cp /usr/bin/pomotui-service "/home/$user/.local/bin/pomotui-service"
            if [[ "$state" == conflict ]]; then
                conflict_path="/home/$user/.config/systemd/user/pomotui.service"
                sed 's|/usr/bin/pomotui-service|/home/pomotui-acceptance/.local/bin/pomotui-service|' /usr/lib/systemd/user/pomotui.service > "$conflict_path"
            else
                conflict_path="/home/$user/.config/systemd/user/pomotui.service.d/source.conf"
                mkdir -p "$(dirname "$conflict_path")"
                printf '[Service]\nExecStart=\nExecStart=/home/%s/.local/bin/pomotui-service\n' "$user" > "$conflict_path"
            fi
            sha256sum "$conflict_path" > "$lifecycle_dir/$state-preserved.sha256"
            as_user systemctl --user daemon-reload
            as_user systemctl --user start pomotui.service
            conflict_pid=$(as_user systemctl --user show pomotui.service -p MainPID --value)
            as_user pomotui status --json > "$lifecycle_dir/$state-source-status.json" ;;

    esac
    as_user systemctl --user show pomotui.service > "$lifecycle_dir/$state-before-unit.txt"
    pacman -U --noconfirm --ask=4 "$candidate" 2>&1 | tee "$lifecycle_dir/$state-upgrade.log"
    case "$state" in
        disabled)
            ! as_user systemctl --user is-enabled --quiet pomotui.socket
            ! as_user systemctl --user is-active --quiet pomotui.service
            ! as_user systemctl --user is-active --quiet pomotui.socket ;;
        conflict|dropin)
            sha256sum --check "$lifecycle_dir/$state-preserved.sha256"
            [[ "$(as_user systemctl --user show pomotui.service -p MainPID --value)" == "$conflict_pid" ]]
            [[ "$(readlink "/proc/$conflict_pid/exe")" == "/home/$user/.local/bin/pomotui-service" ]]
            as_user pomotui status --json > "$lifecycle_dir/$state-preserved-status.json"
            grep -F "$conflict_path" "$lifecycle_dir/$state-upgrade.log"
            grep -F 'conflict' "$lifecycle_dir/$state-upgrade.log" ;;
        *)
            as_user systemctl --user is-active pomotui.service
            pid=$(as_user systemctl --user show pomotui.service -p MainPID --value)
            [[ "$(sha256sum "/proc/$pid/exe" | cut -d' ' -f1)" == "$(sha256sum /usr/bin/pomotui-service | cut -d' ' -f1)" ]]
            as_user pomotui status --json > "$lifecycle_dir/$state-after.json" ;;
    esac
    echo "PASS: lifecycle $state"
done
