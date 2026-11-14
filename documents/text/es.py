# -*- coding: utf-8 -*-
"""Español — el texto de referencia del libro de NikPak."""

MANUAL = [
    ("chap", "Qué es NikPak"),
    ("p", "NikPak es un archivador rápido y cifrado para el formato **.nik**, y **unNik** es el "
          "navegador de archivos gratuito que abre esos archivos en cualquier máquina. Juntos "
          "sustituyen al zip convencional en los dos puntos en que este es más débil: el tamaño y "
          "la confianza."),
    ("img", "01-main.png", "La ventana de NikPak, a la espera de que se suelten archivos."),
    ("p", "Comprime con **Zstandard**, **LZMA2** y **PPMd**, recomprime bit a bit el **Deflate** "
          "interno de los archivos de Office y los PDF, cifra con **Argon2id + XChaCha20-Poly1305** "
          "y lo envuelve en una ventana cuya apariencia se puede cambiar. Los archivos se comprimen "
          "en todos los núcleos, un archivo puede dividirse en volúmenes o convertirse en un `.exe` "
          "autoextraíble, y se pueden adjuntar una imagen de portada y un aviso legal, que todo el "
          "mundo debe aceptar antes de extraer."),
    ("h3", "Los dos programas"),
    ("ul", [
        "**nikpak** — el archivador: una ventana sobre la que se sueltan archivos y una línea de comandos con sintaxis al estilo de RAR (`nikpak a -p secret.nik Photos\\`). Shareware: gratuito para particulares, con licencia por puesto para empresas.",
        "**unNik** — el navegador de archivos gratuito: abre archivos `.nik` en una ventana, recorre carpetas, busca, visualiza texto y extrae. Gratuito para todos, para siempre y sin registro.",
    ]),
    ("p", "Ambos están escritos en Rust y funcionan en Windows, macOS y Linux. El formato `.nik` es "
          "abierto y está documentado en la Parte II, así que un archivo creado hoy puede leerlo un "
          "programa escrito mañana."),
    ("h3", "Lo que aporta"),
    ("ul", [
        "**Archivos más pequeños.** En el nivel máximo, los datos binarios pasan por LZMA2 y el texto por PPMd, elegido por bloque — un 38,8 % menos que RAR 7 en fotografías y un 12–15 % en audio.",
        "**Cifrado real.** Derivación de clave Argon2id y XChaCha20-Poly1305; los nombres de archivo también se cifran, y una contraseña incorrecta o un archivo manipulado se detectan en lugar de extraerse a medias en silencio.",
        "**Multinúcleo.** `-m5` divide el archivo en bloques equilibrados y comprime cada uno en su propio hilo; `-mt<N>` es el mando.",
        "**Filtros multimedia sin pérdida.** WAV, AIFF, MOD y BMP se comprimen a prueba con filtros específicos y se conserva el resultado más pequeño.",
        "**Paquetes.** Empaquete archivos en un paquete `.nik` cifrado y vendible, con un nombre, una descripción, una portada cuadrada y una licencia que hay que aprobar.",
    ]),

    ("chap", "Instalación y el primer archivo"),
    ("p", "Ejecute el instalador o use el ejecutable portable. El instalador añade `nikpak` y "
          "`unNik` al menú de aplicaciones y a la línea de comandos. En macOS y Linux, copie ambos "
          "binarios en `/usr/local/bin`."),
    ("h3", "Desde la ventana"),
    ("p", "Inicie NikPak y **suelte archivos o carpetas sobre la ventana**. El cuadro de arrastre "
          "pregunta dónde escribir el archivo y con qué ajustes; a continuación, el archivo se "
          "construye en la ventana, con una barra de progreso por archivo y otra total. Soltar un "
          "archivo hace lo contrario: abre el cuadro Extraer."),
    ("img", "02-drop.png", "El cuadro de arrastre: qué empaquetar, dónde escribirlo y el nivel."),
    ("h3", "Desde la línea de comandos"),
    ("code", "nikpak a backup.nik C:\\Docs\\*.txt Photos\\        # create an archive\n"
             "nikpak a -m5 -mt8 big.nik Projects\\            # maximum compression, 8 threads\n"
             "nikpak a -p secret.nik Taxes2026\\              # encrypted, asks for the password\n"
             "nikpak x backup.nik C:\\Restore\\               # extract with paths\n"
             "nikpak l backup.nik                            # list\n"
             "nikpak t secret.nik                            # test (asks for the password)"),
    ("p", "`nikpak -help` muestra la referencia completa, incluidas las opciones que este capítulo "
          "no utiliza. Hay una compilación solo de línea de comandos (sin ventana) para servidores."),

    ("chap", "Niveles de compresión y velocidad"),
    ("img", "04-compress.png", "El cuadro de compresión: nivel, hilos y la estimación."),
    ("p", "Seis niveles, de **Almacenar** a **Máximo**. El valor predeterminado es el máximo, "
          "porque un archivo suele escribirse una vez y leerse muchas — pero el nivel es una "
          "elección, no una ley, y la estimación del cuadro le dice cuánto cuesta antes de "
          "comprometerse."),
    ("table", "Los niveles", ["Nivel", "Qué hace", "Cuándo usarlo"], [
        ["Almacenar", "Sin compresión, solo el contenedor y el cifrado", "Medios ya comprimidos que quiera en un solo archivo"],
        ["El más rápido / Rápido", "Zstandard multihilo", "Carpetas enormes, trabajo interactivo"],
        ["Medio", "Zstandard con una ventana mayor", "La opción de cada día"],
        ["Alto", "LZMA2 con PPMd para texto", "Documentos, código fuente, registros"],
        ["Máximo", "LZMA2 / PPMd por bloque, filtros multimedia, recompresión de Deflate", "Archivos que va a conservar y enviar"],
    ], [18, 42, 40], [0]),
    ("p", "**Hilos** es `-mt<N>`, o *Automático* en la ventana. `-m5` ya divide el archivo en "
          "bloques equilibrados y comprime cada uno en su propio hilo; el trabajo por archivo "
          "—pruebas multimedia, recompresión de Deflate— se ejecuta en una canalización en todos "
          "los niveles."),
    ("note", "Los documentos de Office (`docx`, `xlsx`, `pptx` …), los PDF y los archivos `.gz` ya "
             "están comprimidos con Deflate en su interior. NikPak descodifica esos flujos internos, "
             "comprime mucho mejor el contenido sin comprimir y reconstruye los flujos originales "
             "**bit a bit** al extraer: el archivo es más pequeño y el archivo que recupera es el "
             "que metió."),

    ("chap", "Contraseñas y cifrado"),
    ("p", "Una contraseña cifra el archivo de extremo a extremo: **Argon2id** deriva la clave, "
          "**XChaCha20-Poly1305** cifra el contenido y **los nombres de los archivos también se "
          "cifran**. Tanto una contraseña incorrecta como un archivo manipulado se detectan; un "
          "archivo que no supera su comprobación se señala como dañado en lugar de extraerse como "
          "basura."),
    ("ul", [
        "`-p` pide la contraseña dos veces al crear y una al leer; nunca se muestra en pantalla.",
        "Las contraseñas nunca se guardan en el archivo, y NikPak no conserva ninguna entre ejecuciones.",
        "La misma contraseña protege los volúmenes divididos y el `.exe` autoextraíble.",
        "No hay recuperación: una contraseña perdida es un archivo perdido, que es precisamente la finalidad de cifrarlo.",
    ]),

    ("chap", "Volúmenes divididos y archivos autoextraíbles"),
    ("img", "06-split.png", "El cuadro Dividir: tamaños de volumen y la opción autoextraíble."),
    ("p", "`-v` divide un archivo en volúmenes de un tamaño elegido — `nikpak a -v700m video.nik "
          "Movies\\` escribe `video.nik`, `video.n01`, `video.n02` … Para extraer, abra el **primer** "
          "volumen y el resto se leen automáticamente; los demás no se abren a mano."),
    ("p", "`-sfx` escribe un **`.exe` autoextraíble**: unNik con el archivo adjunto, de modo que el "
          "destinatario lo descomprime sin tener nada instalado. Es el formato para enviar a quien "
          "no tiene NikPak, y puede llevar una contraseña y un aviso legal como cualquier otro "
          "archivo."),

    ("chap", "Portadas, avisos legales y paquetes"),
    ("p", "`-cover art.png` adjunta una imagen —cuadrada, almacenada a 1024 × 1024— y `-legal "
          "terms.txt` adjunta un aviso. unNik muestra ambos antes de extraer nada, y no se lee nada "
          "hasta que el lector acepta. Un `cover.jpg` o un `terms.txt` junto a los elementos que se "
          "empaquetan se toma automáticamente."),
    ("img", "05-package.png", "El cuadro Crear paquete: nombre, descripción, portada y licencia."),
    ("p", "Los **paquetes** llevan la misma idea más lejos: un archivo `.nik` con un nombre, una "
          "descripción, una portada cuadrada y una licencia que el destinatario aprueba — el gemelo "
          "en NikPak de un banco de sonidos, hecho para venderse o publicarse. *Crear paquete* y "
          "*Abrir paquete* están en el menú del gestor de archivos."),

    ("chap", "unNik, el navegador gratuito"),
    ("img", "07-unnik.png", "unNik recorriendo un archivo: carpetas a la izquierda, archivos en el centro, la vista previa a la derecha."),
    ("p", "unNik es gratuito para todos, para siempre y sin registro — es lo que hace seguro enviar "
          "un archivo `.nik`: el destinatario no necesita nada más que esto. Abre un archivo en una "
          "ventana, recorre carpetas, busca por nombre, visualiza texto, imágenes y la portada, y "
          "extrae."),
    ("ul", [
        "**Examine antes de extraer** — no se escribe nada en el disco hasta que lo pida.",
        "**Busque** entre los nombres de archivo del archivo y ordene por nombre, tamaño, fecha o tipo.",
        "**Visualice** texto e imágenes del interior del archivo sin descomprimirlos.",
        "**Extraiga** todo, una carpeta o la selección en la carpeta que elija.",
        "**Se le pedirá la contraseña** solo para los archivos que la tengan; el resto se abre en silencio.",
    ]),

    ("chap", "Pieles, idiomas y registro"),
    ("img", "08-skins.png", "El cuadro de pieles junto a la ventana: 89 pieles, elegidas con una vista previa en vivo."),
    ("p", "**89 pieles**, de Amiga a ZX Spectrum, con una vista previa en vivo junto a la ventana. "
          "Diez idiomas de interfaz — English, 中文, हिन्दी, Español, العربية, Français, বাংলা, "
          "Português, Русский y Bahasa Indonesia — elegidos con el botón de la bandera."),
    ("p", "nikpak es shareware: gratuito para particulares y con licencia por puesto para empresas. "
          "*Ayuda ▸ Registrar* toma el nombre, el correo electrónico y la clave exactamente como "
          "figuran en la licencia y los comprueba **sin conexión**. unNik no necesita ninguna "
          "licencia."),
    ("img", "09-register.png", "El cuadro de registro."),
    ("table", "Teclas que utilizará", ["Tecla", "Acción"], [
        ["F1 · F10", "Ayuda · pieles"],
        ["Ctrl+O · Ctrl+N", "Abrir un archivo · nuevo archivo"],
        ["Ctrl+E", "Extraer"],
        ["Ctrl+T", "Probar el archivo"],
        ["F5", "Actualizar"],
        ["Ctrl+Shift+L", "Idioma de la interfaz"],
    ], [34, 66], [0]),
]

TECH = [
    ("chap", "Arquitectura"),
    ("p", "NikPak es un espacio de trabajo de Rust con dos binarios sobre una sola biblioteca: "
          "`nikpak` (el archivador, con ventana y línea de comandos) y `unNik` (el navegador). La "
          "biblioteca posee el formato, los códecs y la criptografía, de modo que los dos programas "
          "no pueden discrepar sobre lo que significa un archivo."),
    ("table", "Las piezas", ["Parte", "Qué hace"], [
        ["El contenedor", "Cabecera, índice de bloques, compresor por bloque y tabla de nombres cifrada"],
        ["Los compresores", "Zstandard (niveles rápidos), LZMA2 y PPMd (máximo), por bloque"],
        ["El recompresor de Deflate", "Descodifica los flujos internos de archivos de Office, PDF y gzip, recomprime el contenido sin comprimir y los reconstruye bit a bit"],
        ["Los filtros multimedia", "Compresión de prueba de WAV, AIFF, MOD y BMP, conservando el resultado más pequeño"],
        ["El códec de audio NAC", "Una ruta de audio sin pérdida para los filtros multimedia"],
        ["La criptografía", "Derivación de clave Argon2id, XChaCha20-Poly1305, cabeceras autenticadas"],
        ["unNik", "La misma biblioteca, de solo lectura, con la interfaz del navegador"],
    ], [26, 74], [0]),
    ("p", "La interfaz gráfica es egui, y por eso la ventana es un solo ejecutable sin ningún "
          "entorno de ejecución que instalar: el archivador y el navegador se distribuyen como "
          "archivos únicos, y el `.exe` autoextraíble es unNik con el archivo adjunto."),

    ("chap", "El formato .nik"),
    ("p", "Un archivo consta de una **cabecera**, un **índice de bloques**, los **bloques** y una "
          "**tabla de nombres cifrada**. La cabecera lleva la versión del formato, los indicadores, "
          "los parámetros de Argon2id y la sal; el índice enumera cada bloque con su compresor, su "
          "desplazamiento y sus tamaños comprimido y sin comprimir."),
    ("ul", [
        "**Los bloques son independientes.** Eso es lo que hace posible la compresión multihilo y lo que hace que un bloque dañado cueste un archivo en lugar del archivo completo.",
        "**Los nombres están en su propia tabla**, cifrada junto con el contenido, así que un archivo no revela ni el contenido ni la forma de lo que contiene.",
        "**Autenticado.** La cabecera y el índice están cubiertos por la misma clave, así que un índice manipulado se detecta antes de escribir nada.",
        "**Versionado.** Un formato más reciente se rechaza con un mensaje en lugar de leerse a medias.",
        "**Los volúmenes** son el mismo flujo cortado en los límites de bloque, así que cualquier volumen puede ser el que se abra, y el resto se localiza por su nombre.",
    ]),

    ("chap", "El motor de compresión"),
    ("p", "El motor elige un compresor **por bloque** y no por archivo, porque los datos de una "
          "misma carpeta rara vez tienen un único carácter: una fotografía y un archivo de código "
          "fuente en el mismo archivo no deberían pasar a la fuerza por la misma ruta. En el nivel "
          "máximo, los datos binarios van a LZMA2 y el texto a PPMd, y la elección queda registrada "
          "en el índice de bloques para que la extracción no tenga que adivinar."),
    ("p", "La **recompresión de Deflate** es la otra mitad de la ganancia de tamaño. Los documentos "
          "de Office, los PDF y los archivos `.gz` son flujos de Deflate dentro de un contenedor; "
          "NikPak descodifica el flujo interno, comprime el contenido sin comprimir con un "
          "algoritmo más potente y, al extraer, reconstruye el flujo de Deflate interno **byte a "
          "byte**, incluidos sus límites de bloque — el archivo que sale es idéntico al que entró y "
          "el archivo resultante es mucho más pequeño que uno que lo hubiera almacenado."),
    ("note", "Los filtros multimedia funcionan por prueba: WAV, AIFF, MOD y BMP se comprimen con la "
             "ruta general y con un filtro específico (predicción adaptativa, reorganización por "
             "planos de bytes, un predictor de píxeles), y se conserva el resultado **más "
             "pequeño**. El filtro cuesta una segunda pasada y aporta un 12–15 % en audio y hasta "
             "un 38,8 % frente a RAR 7 en fotografías."),

    ("chap", "El cifrado y por qué resiste a la computación cuántica"),
    ("p", "Una contraseña pasa por **Argon2id** — intensivo en memoria, así que una GPU o un ASIC "
          "apenas aportan ventaja — y la clave resultante cifra con **XChaCha20-Poly1305**, un "
          "cifrador de flujo autenticado con un nonce de 192 bits, lo bastante amplio para elegir un "
          "nonce aleatorio por archivo sin colisión."),
    ("ul", [
        "**Solo claves simétricas de 256 bits.** En el formato no hay RSA ni curvas elípticas, así que no hay ningún intercambio de clave pública que un futuro ordenador cuántico pueda romper: la seguridad descansa en el cifrador simétrico y en la contraseña.",
        "**Autenticado, no solo cifrado.** Poly1305 cubre el contenido; la cabecera y el índice se autentican con la misma clave, así que un archivo modificado falla en lugar de descodificarse como otra cosa.",
        "**Nombres de archivo cifrados.** La tabla de nombres no es una lista en claro — un archivo no revela lo que contiene.",
        "**Sin ninguna ruta de recuperación**, por diseño: nada en el programa puede descifrar un archivo sin su contraseña.",
    ]),
    ("note", "La afirmación es deliberadamente limitada: **el formato no tiene ningún paso de clave "
             "pública que romper**. Una contraseña débil sigue siendo una contraseña débil, y por "
             "eso la derivación de clave es intensiva en memoria en lugar de un simple hash."),

    ("chap", "Pruebas comparativas, endurecimiento y compilación"),
    ("h3", "Frente a RAR, 7-Zip y ZIP"),
    ("table", "Lo que midieron las pruebas", ["Corpus", "Frente a RAR 7.00", "Frente a 7-Zip 23.01", "Frente a ZIP"], [
        ["Fotografías", "un 38,8 % menos", "por delante", "muy por delante"],
        ["Audio (WAV, AIFF)", "un 12–15 % menos", "por delante", "muy por delante"],
        ["Documentos de Office y PDF", "por delante, gracias a la recompresión de Deflate", "por delante", "muy por delante"],
        ["Carpetas mixtas", "por delante en máximo", "igualado en alto", "muy por delante"],
        ["Velocidad en los niveles rápidos", "más rápido", "igual", "más lento que Almacenar, por diseño"],
    ], [26, 26, 24, 24], [0]),
    ("p", "Las cifras proceden del corpus de pruebas publicado y los scripts están en el "
          "repositorio, así que pueden volver a ejecutarse en lugar de creerse. El resumen honesto: "
          "con compresión máxima NikPak gana en medios y en archivos de Office, está igualado con "
          "7-Zip en datos mixtos y no pretende ganar ninguna carrera de velocidad en los niveles "
          "rápidos."),
    ("h3", "Endurecimiento"),
    ("ul", [
        "**Análisis con comprobación de límites.** Cada longitud del archivo se valida contra los bytes restantes antes de usarla; un archivo mal formado es un error, no un fallo.",
        "**La extracción está confinada** a la carpeta de destino: una entrada llamada `..\\..\\windows\\system32` se rechaza y no se escribe.",
        "**Nada de código unsafe en los analizadores** siempre que pueda evitarse, y donde no se puede, los invariantes están documentados junto al bloque.",
        "**Las pruebas incluyen archivos mal formados** — cabeceras truncadas, sumas de comprobación erróneas, un índice de bloques que apunta más allá del final —, porque un formato que solo ve archivos válidos es un formato que nunca se ha probado.",
    ]),
    ("h3", "Compilación"),
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
    ("p", "Los scripts de publicación mantienen **una sola versión en un solo lugar**: "
          "`buildwin.ps1` escribe el nuevo número en `Cargo.toml`, `Cargo.lock`, "
          "`website/site.json` y `website/version.json` antes de compilar, y lo restaura si la "
          "compilación falla; `buildmac.ps1` y `buildunix.ps1` leen el mismo `Cargo.toml` y se "
          "niegan a compilar cuando el código publicado lleva un número distinto."),
    ("p", "El script de cada plataforma termina escribiendo `NikPak-<version>-<Platform>.zip` con "
          "el instalador, `unNik`, un `README.txt` y los once manuales en una carpeta "
          "`Documentation`, de modo que el sitio ofrece una sola descarga por plataforma y nada "
          "suelto."),

    ("chap", "Este libro"),
    ("p", "Los once PDF se generan a partir de `documents/text/<code>.py` con "
          "`documents/build_docs.py`, usando el mismo motor de composición que los demás productos "
          "de Nikkoid (`..\\nkdocs\\nkdocs.py`). Ejecute `python documents/build_docs.py` después de "
          "cambiar el texto; `scripts\\build-docs.ps1` lo hace antes de empaquetar los zip."),
]
