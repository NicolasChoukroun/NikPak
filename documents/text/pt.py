# -*- coding: utf-8 -*-
"""Português europeu — o texto de referência do livro NikPak."""

MANUAL = [
    ("chap", "O que é o NikPak"),
    ("p", "O NikPak é um arquivador rápido e cifrado para o formato **.nik**, e o **unNik** é o "
          "navegador de arquivos gratuito que abre esses arquivos em qualquer máquina. Juntos, "
          "substituem o zip comum nos dois pontos em que ele é mais fraco: o tamanho e a confiança."),
    ("img", "01-main.png", "A janela do NikPak, à espera de algo para largar."),
    ("p", "Comprime com **Zstandard**, **LZMA2** e **PPMd**, recomprime o **Deflate** dentro de "
          "ficheiros do Office e PDF bit a bit, cifra com **Argon2id + XChaCha20-Poly1305** e "
          "envolve tudo numa janela que pode ter temas. Os ficheiros são comprimidos em todos os "
          "núcleos, um arquivo pode ser dividido em volumes ou transformado num `.exe` "
          "autoextraível, e é possível anexar uma imagem de capa e uma declaração legal, que todos "
          "têm de aceitar antes de extrair."),
    ("h3", "Os dois programas"),
    ("ul", [
        "**nikpak** — o arquivador: uma janela onde larga ficheiros, e uma linha de comandos com sintaxe ao estilo do RAR (`nikpak a -p secret.nik Photos\\`). Shareware: gratuito para particulares, licenciado por posto para empresas.",
        "**unNik** — o navegador de arquivos gratuito: abre arquivos `.nik` numa janela, percorre pastas, pesquisa, visualiza texto e extrai. Gratuito para todos, para sempre, sem registo.",
    ]),
    ("p", "Ambos estão escritos em Rust e funcionam em Windows, macOS e Linux. O formato `.nik` é "
          "aberto e está documentado na Parte II, pelo que um arquivo criado hoje pode ser lido por "
          "um programa escrito amanhã."),
    ("h3", "O que ganha com isso"),
    ("ul", [
        "**Arquivos mais pequenos.** No nível máximo, os dados binários passam por LZMA2 e o texto por PPMd, escolhido bloco a bloco — 38,8 % mais pequenos do que o RAR 7 em fotografias, 12–15 % em áudio.",
        "**Cifra a sério.** Derivação de chave Argon2id e XChaCha20-Poly1305; os nomes dos ficheiros também são cifrados, e uma palavra-passe errada ou um arquivo adulterado é detetado em vez de ser silenciosamente extraído pela metade.",
        "**Multinúcleo.** `-m5` divide o arquivo em blocos equilibrados e comprime cada um na sua própria thread; `-mt<N>` é o regulador.",
        "**Filtros multimédia sem perdas.** WAV, AIFF, MOD e BMP são comprimidos por tentativa com filtros concebidos para o efeito, e mantém-se o resultado mais pequeno.",
        "**Pacotes.** Empacote ficheiros num pacote `.nik` cifrado e vendável, com um nome, uma descrição, uma capa quadrada e uma licença a aprovar.",
    ]),

    ("chap", "Instalação e o primeiro arquivo"),
    ("p", "Execute o instalador ou use o executável portátil. O instalador adiciona o `nikpak` e o "
          "`unNik` ao menu de aplicações e à linha de comandos. Em macOS e Linux, copie ambos os "
          "binários para `/usr/local/bin`."),
    ("h3", "A partir da janela"),
    ("p", "Inicie o NikPak e **largue ficheiros ou pastas na janela**. A caixa de largar pergunta "
          "onde escrever o arquivo e com que definições; o arquivo é então criado na janela, com "
          "uma barra de progresso por ficheiro e o total. Largar um arquivo faz o contrário — abre "
          "a caixa Extrair."),
    ("img", "02-drop.png", "A caixa de largar: o que empacotar, onde escrever e o nível."),
    ("h3", "A partir da linha de comandos"),
    ("code", "nikpak a backup.nik C:\\Docs\\*.txt Photos\\        # create an archive\n"
             "nikpak a -m5 -mt8 big.nik Projects\\            # maximum compression, 8 threads\n"
             "nikpak a -p secret.nik Taxes2026\\              # encrypted, asks for the password\n"
             "nikpak x backup.nik C:\\Restore\\               # extract with paths\n"
             "nikpak l backup.nik                            # list\n"
             "nikpak t secret.nik                            # test (asks for the password)"),
    ("p", "`nikpak -help` imprime toda a referência, incluindo as opções que este capítulo não usa. "
          "Existe uma compilação só de linha de comandos (sem janela) para servidores."),

    ("chap", "Níveis de compressão e velocidade"),
    ("img", "04-compress.png", "A caixa de compressão: nível, threads e a estimativa."),
    ("p", "Seis níveis, de **Guardar** a **Máximo**. O predefinido é o máximo, porque um arquivo "
          "costuma ser escrito uma vez e lido muitas vezes — mas um nível é uma escolha, não uma "
          "lei, e a estimativa na caixa diz-lhe quanto custa antes de decidir."),
    ("table", "Os níveis", ["Nível", "O que faz", "Quando usar"], [
        ["Guardar", "Sem compressão, apenas o contentor e a cifra", "Multimédia já comprimida que quer num único ficheiro"],
        ["Mais rápido / Rápido", "Zstandard com múltiplas threads", "Pastas enormes, trabalho interativo"],
        ["Médio", "Zstandard com uma janela maior", "A escolha do dia a dia"],
        ["Alto", "LZMA2 com PPMd no texto", "Documentos, código-fonte, registos"],
        ["Máximo", "LZMA2 / PPMd por bloco, filtros multimédia, recompressão de Deflate", "Arquivos que vai guardar e enviar"],
    ], [18, 42, 40], [0]),
    ("p", "**Threads** é `-mt<N>`, ou *Automático* na janela. O `-m5` já divide o arquivo em blocos "
          "equilibrados e comprime cada um na sua própria thread; o trabalho por ficheiro — "
          "tentativas multimédia, recompressão de Deflate — corre numa pipeline em todos os níveis."),
    ("note", "Os documentos do Office (`docx`, `xlsx`, `pptx` …), os PDF e os ficheiros `.gz` já "
             "estão comprimidos com Deflate no interior. O NikPak descodifica esses fluxos internos, "
             "comprime muito melhor o conteúdo simples e reconstrói os fluxos originais **bit a "
             "bit** na extração — o arquivo fica mais pequeno e o ficheiro que recebe é o ficheiro "
             "que colocou."),

    ("chap", "Palavras-passe e cifra"),
    ("p", "Uma palavra-passe torna o arquivo cifrado de ponta a ponta: o **Argon2id** deriva a "
          "chave, o **XChaCha20-Poly1305** cifra o conteúdo, e os **nomes dos ficheiros também são "
          "cifrados**. Uma palavra-passe errada e um arquivo adulterado são ambos detetados — um "
          "arquivo que falha a verificação é comunicado como danificado em vez de ser extraído para "
          "lixo."),
    ("ul", [
        "`-p` pede a palavra-passe duas vezes ao criar e uma vez ao ler; nunca é mostrada.",
        "As palavras-passe nunca são guardadas no arquivo, e o NikPak nunca guarda nenhuma entre execuções.",
        "A mesma palavra-passe protege os volumes divididos e o `.exe` autoextraível.",
        "Não há recuperação: uma palavra-passe perdida é um arquivo perdido, que é precisamente o objetivo de o cifrar.",
    ]),

    ("chap", "Volumes divididos e arquivos autoextraíveis"),
    ("img", "06-split.png", "A caixa Dividir: tamanhos dos volumes e a opção autoextraível."),
    ("p", "`-v` divide um arquivo em volumes de um tamanho à escolha — `nikpak a -v700m video.nik "
          "Movies\\` escreve `video.nik`, `video.n01`, `video.n02` … Para extrair, abra o **primeiro** "
          "volume e os restantes são lidos automaticamente; os outros não são abertos à mão."),
    ("p", "`-sfx` escreve um **`.exe` autoextraível**: o unNik com o arquivo anexado, para que o "
          "destinatário o desempacote sem ter nada instalado. É o formato a enviar a quem não tem o "
          "NikPak, e pode transportar uma palavra-passe e uma declaração legal como qualquer outro "
          "arquivo."),

    ("chap", "Capas, declarações legais e pacotes"),
    ("p", "`-cover art.png` anexa uma imagem — quadrada, guardada a 1024 × 1024 — e `-legal "
          "terms.txt` anexa uma declaração. O unNik mostra ambas antes de extrair o que quer que "
          "seja, e nada é lido até o leitor aceitar. Um `cover.jpg` ou um `terms.txt` ao lado dos "
          "itens a empacotar é apanhado automaticamente."),
    ("img", "05-package.png", "A caixa Criar Pacote: nome, descrição, capa e licença."),
    ("p", "Os **pacotes** levam a mesma ideia mais longe: um arquivo `.nik` com um nome, uma "
          "descrição, uma capa quadrada e uma licença que o destinatário aprova — o gémeo NikPak de "
          "um soundbank, feito para ser vendido ou publicado. *Criar Pacote* e *Abrir Pacote* estão "
          "no menu do gestor de ficheiros."),

    ("chap", "unNik, o navegador gratuito"),
    ("img", "07-unnik.png", "O unNik a percorrer um arquivo: pastas à esquerda, ficheiros ao centro, a pré-visualização à direita."),
    ("p", "O unNik é gratuito para todos, para sempre e sem registo — é o que torna um arquivo "
          "`.nik` seguro de enviar: o destinatário não precisa de mais nada além disto. Abre um "
          "arquivo numa janela, percorre pastas, pesquisa por nome, visualiza texto, imagens e a "
          "capa, e extrai."),
    ("ul", [
        "**Navegue antes de extrair** — nada é escrito no disco até o pedir.",
        "**Pesquise** nos nomes dos ficheiros do arquivo e ordene por nome, tamanho, data ou tipo.",
        "**Visualize** texto e imagens de dentro do arquivo sem os desempacotar.",
        "**Extraia** tudo, uma pasta ou a seleção, para uma pasta à sua escolha.",
        "**Peça a palavra-passe** apenas para arquivos que a tenham; os restantes abrem-se silenciosamente.",
    ]),

    ("chap", "Temas, idiomas e registo"),
    ("img", "08-skins.png", "A caixa de temas ao lado da janela: 89 temas, escolhidos com pré-visualização ao vivo."),
    ("p", "**89 temas**, do Amiga ao ZX Spectrum, com uma pré-visualização ao vivo ao lado da "
          "janela. Dez idiomas de interface — English, 中文, हिन्दी, Español, العربية, Français, বাংলা, "
          "Português, Русский e Bahasa Indonesia — escolhidos com o botão da bandeira."),
    ("p", "O nikpak é shareware: gratuito para particulares, licenciado por posto para empresas. "
          "*Ajuda ▸ Registar* pede o nome, o e-mail e a chave exatamente como na licença e "
          "verifica-os **offline**. O unNik não precisa de licença nenhuma."),
    ("img", "09-register.png", "A caixa de registo."),
    ("table", "Teclas que vai usar", ["Tecla", "Ação"], [
        ["F1 · F10", "Ajuda · temas"],
        ["Ctrl+O · Ctrl+N", "Abrir um arquivo · novo arquivo"],
        ["Ctrl+E", "Extrair"],
        ["Ctrl+T", "Testar o arquivo"],
        ["F5", "Atualizar"],
        ["Ctrl+Shift+L", "Idioma da interface"],
    ], [34, 66], [0]),
]

TECH = [
    ("chap", "Arquitetura"),
    ("p", "O NikPak é um workspace Rust de dois binários sobre uma única biblioteca: o `nikpak` (o "
          "arquivador, janela e linha de comandos) e o `unNik` (o navegador). A biblioteca detém o "
          "formato, os codecs e a criptografia, pelo que os dois programas não podem discordar "
          "sobre o que significa um arquivo."),
    ("table", "As peças", ["Parte", "O que faz"], [
        ["O contentor", "Cabeçalho, índice de blocos, compressor por bloco, a tabela de nomes cifrada"],
        ["Os compressores", "Zstandard (níveis rápidos), LZMA2 e PPMd (máximo), por bloco"],
        ["O recompressor de Deflate", "Descodifica os fluxos internos de ficheiros do Office, PDF e gzip, recomprime o conteúdo simples e reconstrói-os bit a bit"],
        ["Os filtros multimédia", "Compressão por tentativa de WAV, AIFF, MOD e BMP, mantendo o resultado mais pequeno"],
        ["O codec de áudio NAC", "Um percurso de áudio sem perdas para os filtros multimédia"],
        ["A criptografia", "Derivação de chave Argon2id, XChaCha20-Poly1305, cabeçalhos autenticados"],
        ["unNik", "A mesma biblioteca, só de leitura, com a interface do navegador"],
    ], [26, 74], [0]),
    ("p", "A interface gráfica é egui, e é por isso que a janela é um único executável sem runtime "
          "para instalar: o arquivador e o navegador são distribuídos como ficheiros únicos, e o "
          "`.exe` autoextraível é o unNik com o arquivo anexado."),

    ("chap", "O formato .nik"),
    ("p", "Um arquivo é um **cabeçalho**, um **índice de blocos**, os **blocos** e uma **tabela de "
          "nomes cifrada**. O cabeçalho transporta a versão do formato, os indicadores, os "
          "parâmetros do Argon2id e o salt; o índice lista cada bloco com o seu compressor, o seu "
          "deslocamento e os seus tamanhos comprimido e não comprimido."),
    ("ul", [
        "**Os blocos são independentes.** É isso que torna possível a compressão com múltiplas threads, e é isso que faz com que um bloco danificado custe um ficheiro em vez do arquivo.",
        "**Os nomes estão numa tabela própria**, cifrada com o conteúdo, pelo que um arquivo não revela nem o conteúdo nem a forma do que contém.",
        "**Autenticado.** O cabeçalho e o índice são cobertos pela mesma chave, pelo que um índice adulterado é detetado antes de se escrever o que quer que seja.",
        "**Com versionamento.** Um formato mais recente é recusado com uma mensagem em vez de ser lido parcialmente.",
        "**Os volumes** são o mesmo fluxo cortado nos limites dos blocos, pelo que qualquer volume pode ser o que abre, e os restantes são encontrados pelo nome.",
    ]),

    ("chap", "O motor de compressão"),
    ("p", "O motor escolhe um compressor **por bloco** e não por arquivo, porque os dados dentro de "
          "uma pasta raramente têm a mesma natureza: uma fotografia e um ficheiro de código-fonte "
          "no mesmo arquivo não devem ser forçados pelo mesmo caminho. No nível máximo, os dados "
          "binários vão para LZMA2 e o texto para PPMd, e a escolha fica registada no índice de "
          "blocos para que a extração não precise de adivinhar."),
    ("p", "A **recompressão de Deflate** é a outra metade do ganho de tamanho. Os documentos do "
          "Office, os PDF e os ficheiros `.gz` são fluxos Deflate dentro de um contentor; o NikPak "
          "descodifica o fluxo interno, comprime o conteúdo simples com um algoritmo mais forte e, "
          "na extração, reconstrói o fluxo Deflate interno **byte a byte**, incluindo os seus "
          "limites de bloco — pelo que o ficheiro que sai é idêntico ao que entrou, e o arquivo é "
          "muito mais pequeno do que um que o tivesse guardado."),
    ("note", "Os filtros multimédia funcionam por tentativa: WAV, AIFF, MOD e BMP são comprimidos "
             "pelo percurso geral e com um filtro concebido para o efeito (previsão adaptativa, "
             "reordenação de planos de bytes, um preditor de píxeis), e mantém-se o resultado "
             "**mais pequeno**. O filtro custa uma segunda passagem e vale 12–15 % em áudio e até "
             "38,8 % face ao RAR 7 em fotografias."),

    ("chap", "Cifra e a razão da resistência quântica"),
    ("p", "Uma palavra-passe passa pelo **Argon2id** — exigente em memória, pelo que uma GPU ou um "
          "ASIC ganham pouco — e a chave resultante cifra com **XChaCha20-Poly1305**, uma cifra de "
          "fluxo autenticada com um nonce de 192 bits, suficientemente largo para escolher um nonce "
          "aleatório por arquivo sem colisão."),
    ("ul", [
        "**Apenas chaves simétricas de 256 bits.** Não há RSA nem curvas elípticas no formato, pelo que não há troca de chaves públicas que um futuro computador quântico possa quebrar: a segurança assenta na cifra simétrica e na palavra-passe.",
        "**Autenticado, não apenas cifrado.** O Poly1305 cobre o conteúdo; o cabeçalho e o índice são autenticados pela mesma chave, pelo que um arquivo modificado falha em vez de descodificar para outra coisa.",
        "**Nomes de ficheiros cifrados.** A tabela de nomes não é uma lista simples — um arquivo não revela o que contém.",
        "**Sem caminho de recuperação**, por conceção: nada no programa consegue decifrar um arquivo sem a sua palavra-passe.",
    ]),
    ("note", "A afirmação é deliberadamente estreita: **o formato não tem nenhum passo de chave "
             "pública que se possa quebrar**. Uma palavra-passe fraca continua a ser uma "
             "palavra-passe fraca, e é por isso que a derivação de chave é exigente em memória em "
             "vez de um simples hash."),

    ("chap", "Benchmarks, reforço de segurança e compilação"),
    ("h3", "Face ao RAR, ao 7-Zip e ao ZIP"),
    ("table", "O que os testes mediram", ["Conjunto", "Face ao RAR 7.00", "Face ao 7-Zip 23.01", "Face ao ZIP"], [
        ["Fotografias", "38,8 % mais pequeno", "à frente", "muito à frente"],
        ["Áudio (WAV, AIFF)", "12–15 % mais pequeno", "à frente", "muito à frente"],
        ["Documentos do Office e PDF", "à frente, graças à recompressão de Deflate", "à frente", "muito à frente"],
        ["Pastas mistas", "à frente no máximo", "igual no alto", "muito à frente"],
        ["Velocidade nos níveis rápidos", "mais rápido", "igual", "mais lento do que Guardar, por conceção"],
    ], [26, 26, 24, 24], [0]),
    ("p", "Os números vêm do conjunto de testes publicado e os scripts estão no repositório, pelo "
          "que podem ser repetidos em vez de acreditados. O resumo honesto: na compressão máxima, o "
          "NikPak ganha em multimédia e em ficheiros do Office, está igual ao 7-Zip em dados mistos "
          "e não está a tentar ganhar uma corrida de velocidade nos níveis rápidos."),
    ("h3", "Reforço de segurança"),
    ("ul", [
        "**Análise com verificação de limites.** Todos os comprimentos no ficheiro são validados face aos bytes restantes antes de serem usados; um arquivo malformado é um erro, não um crash.",
        "**A extração está confinada** à pasta de destino: uma entrada chamada `..\\..\\windows\\system32` é recusada, não escrita.",
        "**Sem código unsafe nos analisadores** sempre que possa ser evitado e, quando não pode, as invariantes estão documentadas ao lado do bloco.",
        "**Os testes incluem arquivos malformados** — cabeçalhos truncados, somas de verificação erradas, um índice de blocos que aponta para além do fim — porque um formato que só vê ficheiros válidos é um formato que nunca foi testado.",
    ]),
    ("h3", "Compilação"),
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
    ("p", "Os scripts de lançamento mantêm **uma versão num único lugar**: o `buildwin.ps1` escreve "
          "o novo número em `Cargo.toml`, `Cargo.lock`, `website/site.json` e "
          "`website/version.json` antes de compilar e repõe-no se a compilação falhar; o "
          "`buildmac.ps1` e o `buildunix.ps1` leem o mesmo `Cargo.toml` e recusam compilar quando "
          "o código enviado tem um número diferente."),
    ("p", "O script de cada plataforma termina escrevendo `NikPak-<version>-<Platform>.zip` com o "
          "instalador, o `unNik`, um `README.txt` e os onze manuais numa pasta `Documentation`, "
          "para que o sítio ofereça uma única transferência por plataforma e nada solto."),

    ("chap", "Este livro"),
    ("p", "Os onze PDF são gerados a partir de `documents/text/<code>.py` pelo "
          "`documents/build_docs.py`, com o mesmo motor de composição tipográfica dos outros "
          "produtos Nikkoid (`..\\nkdocs\\nkdocs.py`). Execute `python documents/build_docs.py` "
          "depois de alterar o texto; o `scripts\\build-docs.ps1` fá-lo antes de os zip serem "
          "empacotados."),
]
