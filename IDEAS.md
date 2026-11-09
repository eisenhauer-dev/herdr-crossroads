# Zukunftsideen

Ideensammlung noch offener Erweiterungen, kein zugesagter Umfang und keine Implementierungsfreigabe.

## Navigationsansichten

- **Agent-Select:** Nur Panes mit erkanntem Agenten anzeigen. Dieselbe Tabelle, Suche und Navigation wie in Select verwenden; nach Search zur vorherigen Ansicht zurückkehren.
- **Agent-Filter:** Beispielsweise Agents anzeigen, die Aufmerksamkeit benötigen oder fertig sind. Keine Agent-Aktionen allein aufgrund eines Status auslösen.

## Ziele wiederfinden

- **Bookmarks:** Häufig genutzte Workspaces, Tabs oder Panes markieren und wiederfinden. Geschlossene oder nicht eindeutig wiedererkennbare Ziele als nicht verfügbar kennzeichnen; nie stillschweigend ein anderes Ziel fokussieren.
- **Jump back:** Zum zuletzt besuchten Ziel zurückspringen, ohne eine allgemeine Verlaufspalette einzuführen.

## Objekte direkt in der Plugin-Ansicht erstellen

- Workspace erstellen.
- Tab im gewählten Workspace erstellen.
- Pane im gewählten Tab erstellen.

Vor einer Umsetzung jeweils Eingaben, Zielkontext, Fokusverhalten, Fehlerfälle und Rückbau gesondert festlegen. Erstellen und Navigieren bleiben klar getrennt; normales `Enter` auf einer Suchtrefferzeile erstellt nichts.

## Leitplanken

- Ruhige Navigation statt allgemeiner Command-Palette.
- Keine destruktiven Aktionen im Hauptfluss.
- Keine Netzwerkzugriffe als Standardverhalten.
- Bestehende Herdr-Bindings und andere Plugins nur nach gesonderter Zustimmung ändern.
