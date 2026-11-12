# Scrub My Disks (SMD)

[English](README.md)

Une application d'entretien gratuite et open source pour Windows 10 et 11. Elle réunit les meilleurs outils gratuits de santé du PC, avec une analyse guidée et sereine et les matières de Windows 11 (Mica, Segoe UI Variable, icônes Fluent).

> **État : phase 4.** Tous les modules fonctionnent : Entretien express (note de santé complète, analyse planifiée chaque jour ou semaine en arrière-plan), Nettoyage (y compris les fichiers de Windows avec les droits administrateur), Carte de l'espace, Protection, Performances, Applications, Mes encombrants, Santé du matériel (détails SMART et capteurs du processeur avec les droits administrateur), Organisation des disques, Confidentialité et réglages (réversibles, point de restauration d'abord), Boîte à outils (outils gratuits fiables via winget), un mini moniteur dans la zone de notification et Mon activité. La phase 5 apporte les finitions et la première version.

## Principes de sécurité

1. **Regarder avant de toucher.** Chaque module analyse d'abord en lecture seule. Rien ne change avant que vous ayez examiné les résultats et cliqué sur le bouton d'action.
2. **Réversible par défaut.** Les fichiers vont dans la Corbeille. Les programmes au démarrage sont désactivés, pas supprimés. Chaque réglage peut être annulé. Un point de restauration système est créé avant toute modification du système.
3. **Moindre privilège.** L'application tourne sans droits administrateur et ne les demande que pour un lot précis, en expliquant pourquoi.
4. **Pas de poudre aux yeux.** Pas de nettoyeur de registre, pas de « booster de RAM », pas de faux gains de vitesse. Chaque chiffre est mesuré.
5. **Vie privée.** Pas de télémétrie, pas de compte. L'application ne se connecte que pour winget, les mises à jour et les téléchargements de la Boîte à outils, et vous le dit.
6. **Gratuit uniquement.** Chaque dépendance est gratuite et sous licence compatible. Voir [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

## Modules et feuille de route

| Module | Rôle | Phase |
|---|---|---|
| Entretien express | Une analyse, une note de santé sur 100 et les meilleures corrections | 1 |
| Nettoyage | Fichiers temporaires, caches, vidages mémoire, caches des navigateurs | 1 |
| Carte de l'espace | Carte interactive de ce qui remplit un disque | 1 |
| Mon activité | Historique des analyses et actions, avec annulation | 1 |
| Applications | Désinstallation propre avec examen des restes, mises à jour winget | 2 |
| Performances | Démarrage, tâches planifiées, services, durée de démarrage | 2 |
| Mes encombrants | Fichiers volumineux, anciens et en double | 2 |
| Protection | État et analyses de Microsoft Defender, pare-feu, autorisations des applications | 3 |
| Santé du matériel | Températures, santé des disques (SMART), batterie, mémoire | 3 |
| Organisation des disques | Chaque disque à sa place, déplacements sûrs | 3b |
| Confidentialité et réglages | Réglages de confidentialité et d'allègement réversibles | 4 |
| Boîte à outils | Installe des outils spécialisés fiables via winget | 4 |

## Compiler

Prérequis (Windows 10/11) : Node.js 24+, Rust (stable, MSVC), Visual Studio 2022 Build Tools avec la charge de travail **Développement Desktop en C++**, Microsoft Edge WebView2 (déjà présent sur Windows 11), SDK .NET 8 pour l'assistant capteurs (les utilisateurs n'ont besoin que du runtime .NET 8).

```bash
npm install
npm run hwmon          # compiler l'assistant capteurs (une fois, puis après chaque modification de sidecars/hwmon)
npm run tauri dev      # lancer l'application
npm test               # tests unitaires
npm run e2e            # tests de fumée (Microsoft Edge)
npm run tauri build    # installateurs NSIS + MSI
```

Les versions non signées déclenchent un avertissement Windows SmartScreen. La signature via SignPath Foundation est prévue.

## Licence

MIT, voir [LICENSE](LICENSE). Crédits : [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
Inspiré de la disposition de CleanMyMac. Scrub My Disks n'est pas affilié à MacPaw et n'utilise aucun de ses éléments.
