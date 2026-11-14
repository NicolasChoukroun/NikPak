# -*- coding: utf-8 -*-
"""Français — le manuel de l'utilisateur et le guide technique de NikPak."""

MANUAL = [
    ("chap", "Ce qu'est NikPak"),
    ("p", "NikPak est un archiveur rapide et chiffré pour le format **.nik**, et **unNik** est le "
          "navigateur d'archives gratuit qui ouvre ces archives sur n'importe quelle machine. "
          "Ensemble, ils remplacent le zip ordinaire là où il est le plus faible : la taille et la "
          "confiance."),
    ("img", "01-main.png", "La fenêtre de NikPak, en attente d'un dépôt."),
    ("p", "Il compresse avec **Zstandard**, **LZMA2** et **PPMd**, recompresse le **Deflate** "
          "contenu dans les fichiers Office et les PDF au bit près, chiffre avec **Argon2id + "
          "XChaCha20-Poly1305**, et l'enveloppe dans une fenêtre que vous pouvez habiller. Les "
          "fichiers sont compressés sur tous les cœurs, une archive peut être découpée en volumes "
          "ou transformée en `.exe` auto-extractible, et l'on peut y joindre une image de "
          "couverture et un texte de licence, que chacun doit accepter avant l'extraction."),
    ("h3", "Les deux programmes"),
    ("ul", [
        "**nikpak** — l'archiveur : une fenêtre sur laquelle vous déposez des fichiers, et une ligne de commande à la syntaxe de RAR (`nikpak a -p secret.nik Photos\\`). Graticiel : gratuit pour les particuliers, sous licence par poste pour les entreprises.",
        "**unNik** — le navigateur d'archives gratuit : il ouvre les archives `.nik` dans une fenêtre, parcourt les dossiers, effectue des recherches, affiche le texte et extrait. Gratuit pour tous, pour toujours, sans inscription.",
    ]),
    ("p", "Les deux sont écrits en Rust et fonctionnent sous Windows, macOS et Linux. Le format "
          "`.nik` est ouvert et documenté dans la partie II, si bien qu'une archive écrite "
          "aujourd'hui peut être lue par un programme écrit demain."),
    ("h3", "Ce que vous y gagnez"),
    ("ul", [
        "**Des archives plus petites.** Au niveau maximal, les données binaires passent par LZMA2 et le texte par PPMd, chacun choisi par bloc — 38,8 % de moins que RAR 7 sur les photographies, 12–15 % sur l'audio.",
        "**Un vrai chiffrement.** Dérivation de clé Argon2id et XChaCha20-Poly1305 ; les noms de fichiers sont chiffrés eux aussi, et un mot de passe erroné ou une archive altérée sont détectés plutôt que d'être extraits à moitié en silence.",
        "**Multicœur.** `-m5` découpe l'archive en blocs équilibrés et compresse chacun sur son propre thread ; `-mt<N>` est le réglage.",
        "**Des filtres multimédia sans perte.** WAV, AIFF, MOD et BMP sont compressés à l'essai avec des filtres spécialisés et le plus petit résultat est conservé.",
        "**Des packages.** Emballez des fichiers dans un package `.nik` chiffré et vendable, avec un nom, une description, une couverture carrée et une licence à approuver.",
    ]),

    ("chap", "Installation et première archive"),
    ("p", "Lancez l'installateur, ou utilisez l'exécutable portable. L'installateur ajoute "
          "`nikpak` et `unNik` au menu des applications et à la ligne de commande. Sous macOS et "
          "Linux, copiez les deux binaires dans `/usr/local/bin`."),
    ("h3", "Depuis la fenêtre"),
    ("p", "Démarrez NikPak et **déposez des fichiers ou des dossiers sur la fenêtre**. La boîte "
          "de dépôt demande où écrire l'archive et avec quels réglages ; l'archive est alors "
          "construite dans la fenêtre, avec une barre de progression par fichier et le total. "
          "Déposer une archive fait l'inverse — cela ouvre la boîte d'extraction."),
    ("img", "02-drop.png", "La boîte de dépôt : quoi empaqueter, où l'écrire et le niveau."),
    ("h3", "Depuis la ligne de commande"),
    ("code", "nikpak a backup.nik C:\\Docs\\*.txt Photos\\        # create an archive\n"
             "nikpak a -m5 -mt8 big.nik Projects\\            # maximum compression, 8 threads\n"
             "nikpak a -p secret.nik Taxes2026\\              # encrypted, asks for the password\n"
             "nikpak x backup.nik C:\\Restore\\               # extract with paths\n"
             "nikpak l backup.nik                            # list\n"
             "nikpak t secret.nik                            # test (asks for the password)"),
    ("p", "`nikpak -help` affiche la référence complète, y compris les options que ce chapitre "
          "n'utilise pas. Une version en ligne de commande seule (sans fenêtre) est disponible "
          "pour les serveurs."),

    ("chap", "Niveaux de compression et vitesse"),
    ("img", "04-compress.png", "La boîte de compression : niveau, threads et l'estimation."),
    ("p", "Six niveaux, de **Stocker** à **Maximum**. Le niveau par défaut est le maximum, car "
          "une archive est généralement écrite une fois et lue maintes fois — mais un niveau est "
          "un choix, non une loi, et l'estimation dans la boîte vous dit ce qu'il coûte avant de "
          "vous engager."),
    ("table", "Les niveaux", ["Niveau", "Ce qu'il fait", "Quand l'utiliser"], [
        ["Stocker", "Aucune compression, juste le conteneur et le chiffrement", "Des médias déjà compressés que vous voulez dans un seul fichier"],
        ["Le plus rapide / Rapide", "Zstandard multithread", "Dossiers énormes, travail interactif"],
        ["Normale", "Zstandard avec une fenêtre plus grande", "Le choix de tous les jours"],
        ["Élevée", "LZMA2 avec PPMd sur le texte", "Documents, code source, journaux"],
        ["Maximum", "LZMA2 / PPMd par bloc, filtres multimédia, recompression Deflate", "Des archives que vous conserverez et enverrez"],
    ], [18, 42, 40], [0]),
    ("p", "**Threads** se règle avec `-mt<N>`, ou par *Automatique* dans la fenêtre. `-m5` découpe "
          "déjà l'archive en blocs équilibrés et compresse chacun sur son propre thread ; le "
          "travail par fichier — essais multimédia, recompression Deflate — s'exécute en pipeline "
          "à tous les niveaux."),
    ("note", "Les documents Office (`docx`, `xlsx`, `pptx` …), les PDF et les fichiers `.gz` sont "
             "déjà compressés en Deflate à l'intérieur. NikPak décode ces flux internes, compresse "
             "bien mieux le contenu en clair, et reconstruit les flux d'origine **au bit près** à "
             "l'extraction — l'archive est plus petite et le fichier que vous récupérez est le "
             "fichier que vous y avez mis."),

    ("chap", "Mots de passe et chiffrement"),
    ("p", "Un mot de passe rend l'archive chiffrée de bout en bout : **Argon2id** dérive la clé, "
          "**XChaCha20-Poly1305** chiffre le contenu, et les **noms des fichiers sont chiffrés "
          "eux aussi**. Un mot de passe erroné et une archive altérée sont tous deux détectés — "
          "une archive qui échoue à son contrôle est signalée comme endommagée plutôt que d'être "
          "extraite en un amas inutilisable."),
    ("ul", [
        "`-p` demande le mot de passe deux fois à la création, une fois à la lecture ; il n'est jamais affiché.",
        "Les mots de passe ne sont jamais stockés dans l'archive, et NikPak n'en conserve aucun entre deux exécutions.",
        "Le même mot de passe protège les volumes découpés et le `.exe` auto-extractible.",
        "Il n'y a aucune récupération possible : un mot de passe perdu est une archive perdue, et c'est précisément le but du chiffrement.",
    ]),

    ("chap", "Volumes découpés et archives auto-extractibles"),
    ("img", "06-split.png", "La boîte de découpage : tailles de volume et option auto-extractible."),
    ("p", "`-v` découpe une archive en volumes d'une taille choisie — `nikpak a -v700m video.nik "
          "Movies\\` écrit `video.nik`, `video.n01`, `video.n02` … Pour extraire, ouvrez le "
          "**premier** volume et les autres sont lus automatiquement ; les autres ne s'ouvrent "
          "pas à la main."),
    ("p", "`-sfx` écrit un **`.exe` auto-extractible** : unNik avec l'archive ajoutée à la suite, "
          "si bien qu'un destinataire la décompresse sans rien avoir installé. C'est le format à "
          "envoyer à quelqu'un qui n'a pas NikPak, et il peut porter un mot de passe et un texte "
          "de licence comme n'importe quelle autre archive."),

    ("chap", "Couvertures, textes de licence et packages"),
    ("p", "`-cover art.png` joint une image — carrée, stockée en 1024 × 1024 — et `-legal "
          "terms.txt` joint un texte. unNik montre les deux avant d'extraire quoi que ce soit, et "
          "rien n'est lu tant que le lecteur n'a pas accepté. Un `cover.jpg` ou un `terms.txt` à "
          "côté des éléments empaquetés est repris automatiquement."),
    ("img", "05-package.png", "La boîte Créer un package : nom, description, couverture et licence."),
    ("p", "Les **packages** sont la même idée poussée plus loin : une archive `.nik` avec un nom, "
          "une description, une couverture carrée et une licence que le destinataire approuve — "
          "le pendant NikPak d'une banque de sons, faite pour être vendue ou publiée. *Créer un "
          "package* et *Ouvrir un package* se trouvent dans le menu du gestionnaire de fichiers."),

    ("chap", "unNik, le navigateur gratuit"),
    ("img", "07-unnik.png", "unNik parcourant une archive : dossiers à gauche, fichiers au milieu, l'aperçu à droite."),
    ("p", "unNik est gratuit pour tous, pour toujours, sans inscription — c'est ce qui rend une "
          "archive `.nik` sûre à envoyer : le destinataire n'a besoin de rien d'autre. Il ouvre "
          "une archive dans une fenêtre, parcourt les dossiers, recherche par nom, affiche le "
          "texte, les images et la couverture, et extrait."),
    ("ul", [
        "**Parcourir avant d'extraire** — rien n'est écrit sur le disque tant que vous ne le demandez pas.",
        "**Rechercher** dans les noms de fichiers de l'archive, et trier par nom, taille, date ou type.",
        "**Afficher** le texte et les images depuis l'intérieur de l'archive sans les décompresser.",
        "**Extraire** tout, un dossier, ou la sélection, vers un dossier que vous choisissez.",
        "**Demander le mot de passe** uniquement pour les archives qui en ont un ; les autres s'ouvrent en silence.",
    ]),

    ("chap", "Habillages, langues et enregistrement de la licence"),
    ("img", "08-skins.png", "La boîte des habillages à côté de la fenêtre : 89 habillages, choisis avec un aperçu en direct."),
    ("p", "**89 habillages**, de l'Amiga au ZX Spectrum, avec un aperçu en direct à côté de la "
          "fenêtre. Dix langues d'interface — English, 中文, हिन्दी, Español, العربية, Français, "
          "বাংলা, Português, Русский et Bahasa Indonesia — choisies avec le bouton drapeau."),
    ("p", "nikpak est un graticiel : gratuit pour les particuliers, sous licence par poste pour "
          "les entreprises. *Aide ▸ Enregistrer* demande le nom, l'adresse électronique et la clé "
          "exactement comme sur la licence, puis vérifie l'ensemble **hors ligne**. unNik ne "
          "demande aucune licence."),
    ("img", "09-register.png", "La boîte d'enregistrement."),
    ("table", "Les touches que vous utiliserez", ["Touche", "Action"], [
        ["F1 · F10", "Aide · habillages"],
        ["Ctrl+O · Ctrl+N", "Ouvrir une archive · nouvelle archive"],
        ["Ctrl+E", "Extraire"],
        ["Ctrl+T", "Tester l'archive"],
        ["F5", "Rafraîchir"],
        ["Ctrl+Maj+L", "Langue de l'interface"],
    ], [34, 66], [0]),
]

TECH = [
    ("chap", "Architecture"),
    ("p", "NikPak est un espace de travail Rust de deux binaires au-dessus d'une seule "
          "bibliothèque : `nikpak` (l'archiveur, fenêtre et ligne de commande) et `unNik` (le "
          "navigateur). La bibliothèque possède le format, les codecs et la cryptographie, si "
          "bien que les deux programmes ne puissent pas diverger sur ce que signifie une archive."),
    ("table", "Les éléments", ["Élément", "Ce qu'il fait"], [
        ["Le conteneur", "En-tête, index des blocs, compresseur par bloc, la table des noms chiffrée"],
        ["Les compresseurs", "Zstandard (niveaux rapides), LZMA2 et PPMd (maximum), par bloc"],
        ["Le recompresseur Deflate", "Décode les flux internes des fichiers Office, des PDF et de gzip, recompresse le contenu en clair, les reconstruit au bit près"],
        ["Les filtres multimédia", "Compression à l'essai des fichiers WAV, AIFF, MOD et BMP, plus petit résultat conservé"],
        ["Le codec audio NAC", "Un chemin audio sans perte pour les filtres multimédia"],
        ["La cryptographie", "Dérivation de clé Argon2id, XChaCha20-Poly1305, en-têtes authentifiés"],
        ["unNik", "La même bibliothèque, en lecture seule, avec l'interface du navigateur"],
    ], [26, 74], [0]),
    ("p", "L'interface graphique est egui, et c'est pourquoi la fenêtre est un seul exécutable "
          "sans environnement d'exécution à installer : l'archiveur et le navigateur sont livrés "
          "en fichiers uniques, et le `.exe` auto-extractible est unNik avec l'archive ajoutée à "
          "la suite."),

    ("chap", "Le format .nik"),
    ("p", "Une archive est un **en-tête**, un **index des blocs**, les **blocs** et une **table "
          "des noms chiffrée**. L'en-tête porte la version du format, les indicateurs, les "
          "paramètres Argon2id et le sel ; l'index liste chaque bloc avec son compresseur, son "
          "décalage et ses tailles compressée et non compressée."),
    ("ul", [
        "**Les blocs sont indépendants.** C'est ce qui rend possible la compression multithread, et ce qui fait qu'un bloc endommagé coûte un fichier plutôt que l'archive.",
        "**Les noms sont dans leur propre table**, chiffrée avec le contenu, si bien qu'une archive ne révèle ni le contenu ni la structure de ce qu'elle renferme.",
        "**Authentifiée.** L'en-tête et l'index sont couverts par la même clé, de sorte qu'un index altéré est détecté avant que quoi que ce soit ne soit écrit.",
        "**Versionné.** Un format plus récent est refusé avec un message plutôt que lu partiellement.",
        "**Les volumes** sont le même flux coupé aux frontières des blocs, si bien que n'importe quel volume peut être celui que vous ouvrez, et les autres sont retrouvés par leur nom.",
    ]),

    ("chap", "Le moteur de compression"),
    ("p", "Le moteur choisit un compresseur **par bloc** plutôt que par archive, parce que les "
          "données d'un même dossier ont rarement un seul caractère : une photographie et un "
          "fichier source dans la même archive ne devraient pas être forcés de passer par le même "
          "chemin. Au niveau maximal, les données binaires vont à LZMA2 et le texte à PPMd, et le "
          "choix est consigné dans l'index des blocs afin que l'extraction n'ait rien à deviner."),
    ("p", "La **recompression Deflate** est l'autre moitié du gain de taille. Les documents "
          "Office, les PDF et les fichiers `.gz` sont des flux Deflate à l'intérieur d'un "
          "conteneur ; NikPak décode le flux interne, compresse le contenu en clair avec un "
          "algorithme plus puissant, et à l'extraction reconstruit le flux Deflate interne "
          "**octet pour octet**, y compris ses frontières de blocs — si bien que le fichier qui "
          "sort est identique à celui qui est entré, et que l'archive est bien plus petite que "
          "celle qui l'aurait stocké."),
    ("note", "Les filtres multimédia fonctionnent par essai : WAV, AIFF, MOD et BMP sont "
             "compressés avec le chemin général et avec un filtre spécialisé (prédiction "
             "adaptative, réarrangement des plans d'octets, un prédicteur de pixels), et le "
             "résultat le **plus petit** est conservé. Le filtre coûte une seconde passe et "
             "rapporte 12–15 % sur l'audio et jusqu'à 38,8 % face à RAR 7 sur les photographies."),

    ("chap", "Le chiffrement, et pourquoi il résiste au quantique"),
    ("p", "Un mot de passe passe par **Argon2id** — gourmand en mémoire, si bien qu'un GPU ou un "
          "ASIC n'y gagne guère — et la clé obtenue chiffre avec **XChaCha20-Poly1305**, un "
          "chiffrement par flot authentifié à nonce de 192 bits, assez large pour tirer un nonce "
          "aléatoire par archive sans collision."),
    ("ul", [
        "**Clés symétriques de 256 bits uniquement.** Il n'y a ni RSA ni courbe elliptique dans le format, donc aucun échange de clés publiques qu'un futur ordinateur quantique puisse casser : la sécurité repose sur le chiffrement symétrique et sur le mot de passe.",
        "**Authentifié, pas seulement chiffré.** Poly1305 couvre le contenu ; l'en-tête et l'index sont authentifiés par la même clé, si bien qu'une archive modifiée échoue au lieu de se décoder en autre chose.",
        "**Noms de fichiers chiffrés.** La table des noms n'est pas une simple liste — une archive ne révèle pas ce qu'elle contient.",
        "**Aucun recours**, par conception : rien dans le programme ne peut déchiffrer une archive sans son mot de passe.",
    ]),
    ("note", "L'affirmation est délibérément étroite : **le format n'a aucune étape de clé "
             "publique à casser**. Un mot de passe faible reste un mot de passe faible, et c'est "
             "pourquoi la dérivation de clé est gourmande en mémoire plutôt qu'un simple hachage."),

    ("chap", "Comparatifs, durcissement et compilation"),
    ("h3", "Face à RAR, 7-Zip et ZIP"),
    ("table", "Ce que les tests ont mesuré", ["Corpus", "Face à RAR 7.00", "Face à 7-Zip 23.01", "Face à ZIP"], [
        ["Photographies", "38,8 % plus petit", "en tête", "loin devant"],
        ["Audio (WAV, AIFF)", "12–15 % plus petit", "en tête", "loin devant"],
        ["Documents Office et PDF", "en tête, grâce à la recompression Deflate", "en tête", "loin devant"],
        ["Dossiers mixtes", "en tête au maximum", "à égalité au niveau élevé", "loin devant"],
        ["Vitesse aux niveaux rapides", "plus rapide", "à égalité", "plus lent que Stocker, par conception"],
    ], [26, 26, 24, 24], [0]),
    ("p", "Les chiffres proviennent du corpus de test publié et les scripts sont dans le dépôt, "
          "si bien qu'on peut les relancer plutôt que les croire. Le résumé honnête : à "
          "compression maximale, NikPak gagne sur les médias et sur les fichiers Office, est à "
          "égalité avec 7-Zip sur les données mixtes, et ne cherche pas à gagner une course de "
          "vitesse aux niveaux rapides."),
    ("h3", "Durcissement"),
    ("ul", [
        "**Analyse avec vérification des bornes.** Chaque longueur du fichier est validée par rapport aux octets restants avant d'être utilisée ; une archive malformée est une erreur, pas un plantage.",
        "**L'extraction est confinée** au dossier cible : une entrée nommée `..\\..\\windows\\system32` est refusée, pas écrite.",
        "**Aucun code unsafe dans les analyseurs** là où c'est évitable, et là où ça ne l'est pas, les invariants sont documentés à côté du bloc.",
        "**Les tests incluent des archives malformées** — en-têtes tronqués, sommes de contrôle erronées, un index de blocs qui pointe au-delà de la fin — parce qu'un format qui ne voit jamais que des fichiers valides est un format qui n'a jamais été testé.",
    ]),
    ("h3", "Compilation"),
    ("code", "# Windows\n"
             ".\\build.ps1              # dist\\nikpak.exe and dist\\unNik.exe\n"
             ".\\build.ps1 -Test        # run the tests first\n"
             ".\\build.ps1 -Installer   # also the setup (needs Inno Setup 6)\n"
             "\n"
             "# macOS / Linux\n"
             "./build.sh               # dist/nikpak and dist/unNik\n"
             "\n"
             "# command-line only, no window\n"
             "cargo build --release --no-default-features"),
    ("p", "Les scripts de publication gardent **un seul numéro de version à un seul endroit** : "
          "`buildwin.ps1` écrit le nouveau numéro dans `Cargo.toml`, `Cargo.lock`, "
          "`website/site.json` et `website/version.json` avant de compiler et le restaure si la "
          "compilation échoue ; `buildmac.ps1` et `buildunix.ps1` lisent le même `Cargo.toml` et "
          "refusent de compiler quand la source poussée porte un numéro différent."),
    ("p", "Le script de chaque plateforme termine en écrivant `NikPak-<version>-<Platform>.zip` "
          "avec l'installateur, `unNik`, un `README.txt` et les onze manuels dans un dossier "
          "`Documentation`, si bien que le site propose un seul téléchargement par plateforme et "
          "rien en vrac."),

    ("chap", "Ce livre"),
    ("p", "Les onze PDF sont générés à partir de `documents/text/<code>.py` par "
          "`documents/build_docs.py`, avec le même moteur de composition que les autres produits "
          "Nikkoid (`..\\nkdocs\\nkdocs.py`). Lancez `python documents/build_docs.py` après avoir "
          "modifié le texte ; `scripts\\build-docs.ps1` le fait avant que les zips ne soient "
          "empaquetés."),
]
