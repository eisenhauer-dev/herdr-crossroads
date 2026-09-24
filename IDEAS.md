# Zukunftsideen

Ideensammlung, kein zugesagter Umfang und keine Implementierungsfreigabe. Die erste Plugin-Iteration bleibt auf Anzeige, Suche und Fokuswechsel beschränkt.

## Erste mögliche Erweiterung: Mausunterstützung

- Herdr-Nutzer sollen Tabellenzeilen per Maus auswählen und fokussieren können, ohne die Tastaturbedienung zu verlieren.
- Mausereignisse, Verhalten von Doppelklick/Einfachklick, Scrollen, Fokuswechsel und Abbruch erst nach Prüfung der Herdr- und Terminal-API festlegen.
- Die Bedienung muss weiterhin vollständig ohne Maus funktionieren; keine Mausaktion darf unbeabsichtigt einen Pane oder Workspace schließen.

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
