# Vérifications automatiques et matérielles

Ce guide associe chaque promesse documentée à une vérification concrète. Les tests Cargo couvrent le comportement déterministe; seuls les contrôles manuels confirment ce que les deux écrans et les contrôles physiques affichent réellement.

## Vérifications automatisées

| Domaine | Vérification | Couverture représentative |
|---|---|---|
| Style et compilation | `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings` | Formatage et avertissements traités comme erreurs, y compris les tests. |
| Configuration réduite | `cargo check -p maschine-engine --no-default-features` | Le moteur compile quand l'import WAV optionnel est désactivé. |
| Protocole LCD | `cargo test -p maschine-proto` | Fenêtres RGB565 valides, largeur paire, expansion d'un pixel isolé, compteur de paires et reconstruction exacte du framebuffer (`differ_always_reconstructs_exactly`). |
| Envoi LCD | `cargo test -p maschine-hardware` | Complétion et terminaison d'une fenêtre interrompue, y compris le cas de short write capturé dans les logs; rapports HID et retours LEDs. |
| Navigation matérielle | `cargo test -p maschine-hardware selecting_main_view_clears_events_overlay` | Le passage d'EVENTS à une vue principale restaure sa page droite contextuelle. |
| Navigation moteur | `cargo test -p maschine-engine selecting_sampler_restores_contextual_right_screen_after_events` | SAMPLER reprend son écran droit après EVENTS; changement de vue hors ARRANGE quitte le mode Song. |
| Import et restitution WAV | `cargo test -p maschine-engine` | WAV PCM 32 bits signé, état conservé si l'import échoue, waveform mise en cache, isolation des Groups, écriture des mappings dans le projet et rechargement d'un chemin relatif. |
| Mix en cours de lecture | `cargo test -p maschine-engine sound_and_group_mix_edits_reach_active_voices_immediately` | Volume et pan Sound/Group sont répercutés sur les voix déjà déclenchées. |
| Intégration GUI/HID | `cargo test -p maschine-app` | Routage d'événements, transitions de contexte et sonde de reprise RGB565 2×1. |
| Régressions générales | `cargo test --workspace` | Tous les tests unitaires et d'intégration du workspace. |
| Binaire optimisé | `cargo build --workspace --release` | Compilation du binaire et des crates en mode release. |

La CI GitHub exécute formatage, Clippy, compilation moteur sans Sampler, tests workspace et build release à chaque push/PR sur `master` ou `main`.

## Contrôle manuel de la Maschine MK3

L'USB ne fournit pas ici d'accusé de réception des pixels affichés. Après avoir lancé `./target/release/maschine-linux gui --no-audio` avec la MK3 branchée :

1. Vérifie que les deux écrans montrent le contenu de l'application.
2. Ouvre **EVENTS**, puis **SAMPLER** depuis le hardware; l'écran droit doit quitter EVENTS et montrer la page Sampler.
3. Tourne un encodeur une seule fois; la valeur affichée et le paramètre actif doivent changer dès le premier cran.
4. Passe par GUI puis par hardware entre ARRANGE, MIXER et SAMPLER; vérifie que la page droite suit le contexte courant.
5. Dans Sampler, renseigne le chemin d'un WAV, importe-le, puis écoute le pad. Fais `Ctrl+S` (le chemin exact est indiqué dans l'interface), ferme l'application et rouvre le fichier sauvegardé avec `./target/release/maschine-linux gui --project "$HOME/.local/share/maschine-linux/autosave.json" --no-audio`. Le nom, la waveform et le son doivent être restaurés.

La fonctionnalité officielle complète de SAMPLING (RECORD, EDIT, SLICE et édition ZONE) et l'édition des événements ne sont pas couvertes comme implémentées : elles restent hors du périmètre actuel. Voir [workflow.md](workflow.md) pour le détail des écarts et des contrôles disponibles.
