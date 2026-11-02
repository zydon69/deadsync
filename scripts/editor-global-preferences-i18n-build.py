from pathlib import Path
root=Path('/Users/zydon/Sites/feetness/deadsync');base=Path('/tmp/deadsync-source-preferences/base');work=base.parent/'work'
values={
'SourcePreferencesOpen':('Import GrooveAuthor preferences…','Importer les préférences GrooveAuthor…'),
'SourcePreferencesPath':('GrooveAuthor preferences file','Fichier de préférences GrooveAuthor'),
'SourcePreferencesReview':('Read and review the import','Lire et examiner l’import'),
'SourcePreferencesHelp':('Enter the source preferences JSON path. Review all mappings before applying. The original file is preserved.','Saisissez le chemin du JSON source. Examinez les correspondances avant de les appliquer. Le fichier original est conservé.'),
'SourcePreferencesReviewHelp':('Left/Right or PageUp/PageDown: browse report. Tab: choose. Accept the reported limitations before applying.','Gauche/Droite ou Page préc./suiv. : parcourir le rapport. Tab : choisir. Acceptez les limites signalées avant d’appliquer.'),
'SourcePreferencesReportRow':('Import report','Rapport d’import'),
'SourcePreferencesAccept':('Accept the reported adaptations and retained fields','Accepter les adaptations et les champs conservés signalés'),
'SourcePreferencesAcceptance':('Reported limitations accepted: {value}','Limites signalées acceptées : {value}'),
'SourcePreferencesApply':('Apply reviewed preferences','Appliquer les préférences examinées'),
'SourcePreferencesCancel':('Cancel import','Annuler l’import'),
'SourcePreferencesNoReview':('Read the preferences file first.','Lisez d’abord le fichier de préférences.'),
'SourcePreferencesImported':('GrooveAuthor preferences imported. Undo restores the previous settings. Restart applies the saved startup/DPI/DSP choices.','Préférences GrooveAuthor importées. Annuler restaure les réglages précédents. Le redémarrage applique les choix de démarrage, PPP et DSP enregistrés.'),
'SourcePreferencesFailed':('Preferences import failed: {error}','Échec de l’import des préférences : {error}'),
'SourcePreferencesApplied':('applied','appliqué'),
'SourcePreferencesDefaulted':('source default','valeur source par défaut'),
'SourcePreferencesRetained':('retained in the original archive, not consumed','conservé dans l’archive originale, sans consommateur'),
'SourcePreferencesNormalized':('source normalization','normalisation source'),
'SourcePreferencesFallback':('source enum fallback','repli de l’énumération source'),
'SourcePreferencesAdapted':('adapted; raw source value preserved','adapté ; valeur source brute conservée'),
'SourcePreferencesWindowAdapted':('window state archived; advanced panel opens on demand','état de fenêtre archivé ; le panneau avancé s’ouvre à la demande'),
'SourcePreferencesKeyRejected':('binding rejected','raccourci refusé'),
'SourcePreferencesKeyUnavailable':('source command unavailable','commande source indisponible'),
'SourcePreferencesKeyNeutralized':('native extension disabled by the source profile','extension native désactivée par le profil source'),
'SourcePreferencesKeyRescue':('rescue command preserved','commande de secours conservée'),
'SourcePreferencesSummary':('{rows} preference rows. {omitted} further report entries omitted; raw input remains archived.','{rows} lignes de préférences. {omitted} entrées supplémentaires du rapport non affichées ; le contenu brut reste archivé.'),
}
for index,lang in enumerate(('en','fr')):
 rel=Path(f'assets/languages/{lang}.ini');s=(root/rel).read_text();(base/rel).parent.mkdir(parents=True,exist_ok=True);(work/rel).parent.mkdir(parents=True,exist_ok=True);(base/rel).write_text(s)
 # Insert into Edition section, before next section, not at an arbitrary EOF.
 start=s.index('[Edition]');end=s.find('\n[',start+1);end=len(s) if end<0 else end
 s=s[:end]+'\n'+''.join(k+'='+v[index]+'\n' for k,v in values.items())+s[end:];(work/rel).write_text(s)
(root/'patches/editor-global-preferences-i18n.json').write_text(__import__('json').dumps(values,ensure_ascii=False,indent=2)+'\n')
