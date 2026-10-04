# CapyDock

Aplicativo independente para Linux, com **Tauri 2, Nuxt 4, Vue 3, Tailwind 4 e DaisyUI 5**. Incorpora o **CLI oficial da Proton**, sem pedir a senha dentro da interface. Não é um produto da Proton AG.

## Releases

A versão pública começa em **0.1.0**, distribuída somente como **AppImage Linux x86-64**. Commits em `dev` passam pelos testes e compilação. A publicação acontece somente quando as mudanças chegam à branch `release`. Consulte o [fluxo de releases](docs/releases.md) para as regras de Conventional Commits e retomada de falhas.

## Usar

Depois da instalação local:

```sh
capydock
proton-drive version
```

1. Clique em **Conectar conta Proton** e conclua o login no navegador. O CLI usa o cofre de credenciais do Linux; é necessário ter uma sessão de GNOME Keyring, KWallet com Secret Service ou equivalente disponível.
2. Clique em **Adicionar pasta**, escolha a pasta local e use **Explorar** para selecionar ou criar uma pasta no Drive.
3. Escolha os dois sentidos, somente envio ou somente download. Mudanças locais disparam a sincronização automaticamente; o intervalo é usado para buscar mudanças no Drive.
4. Mantenha o aplicativo aberto ou minimizado. **Configurações → Iniciar com o computador** habilita a inicialização na sessão Linux.

**Biblioteca** agora tem abas **Arquivos, Documentos, Fotos e Álbuns**, busca, navegação por pasta e paginação. PDFs, imagens e texto abrem em uma pré-visualização local. Arquivos Office e vídeos podem ser baixados para seus aplicativos. Docs e Sheets abrem diretamente no editor Proton em uma janela integrada, isolada das permissões locais. O login web é independente da sessão do CLI.

Em **Biblioteca → Configurar biblioteca**, selecione uma pasta. O app cria três pareamentos (os nomes iniciais acompanham o idioma escolhido; abaixo, em português):

| Pasta local | Comportamento |
| --- | --- |
| `Arquivos/` | Todas as pastas de `/my-files`, nos dois sentidos. Exclusões podem ser ativadas explicitamente. |
| `Fotos/` | Recebe fotos/vídeos da timeline e envia novos arquivos de mídia locais. |
| `Álbuns/` | Cópia contínua do Drive para o PC, organizada por álbum. Descobre novos álbuns e novas fotos automaticamente. |

A configuração completa exige remover pareamentos anteriores que se sobreponham; remover um pareamento preserva todos os arquivos. Também é possível entrar em um álbum e clicar em **Sincronizar aqui** para receber suas fotos e enviar novas fotos diretamente para ele. Todos os pareamentos monitoram alterações locais e aparecem em **Pastas sincronizadas**. O intervalo configurável é usado para buscar mudanças no Drive.

## O que está implementado

- Explorador de arquivos, índice recursivo de documentos, timeline de fotos e álbuns.
- Leitor PDF.js local com paginação e texto acessível; pré-visualização de imagens e texto, download sem sobrescrever arquivos existentes.
- Configuração de toda a biblioteca, sincronização incremental de fotos e cópia contínua de todos os álbuns.
- Identificadores estáveis nos nomes locais de fotos/álbuns para preservar itens que têm nomes iguais.
- Múltiplos pareamentos de pastas, seleção nativa, navegação e criação de pastas remotas.
- Sincronização incremental de arquivos e diretórios em `/my-files`, incluindo subpastas vazias.
- Comparação de conteúdo local com SHA-256 e acompanhamento das revisões remotas.
- Monitor nativo Linux (inotify): criar, editar, salvar por substituição atômica, renomear ou excluir arquivos/pastas agenda uma sincronização após aproximadamente 2 segundos sem novas alterações.
- Eventos são agrupados por pareamento, preservados durante transferências e pausas, e processados sem executar sincronizações concorrentes. Leituras, backups e diretórios ignorados não provocam ciclos.
- Reconciliação na inicialização, ao ativar um pareamento e ao retomar a pausa; remontagens e substituições da pasta local reconstituem o monitor.
- Se o monitor nativo não estiver disponível, usa verificação local de metadados a cada 2 segundos e informa no histórico. A verificação periódica completa permanece como recuperação.
- Busca periódica de mudanças na nuvem por pasta, pausa global/individual, execução manual e interrupção entre operações.
- Histórico persistido com os últimos 200 eventos, erros e conflitos.
- Cópias de segurança locais antes de substituir um arquivo recebido; uploads alterados criam novas revisões no Drive.
- Identificação da conta para impedir que um pareamento existente seja usado com outra conta.
- Rejeição de pareamentos sobrepostos, caminhos inválidos, arquivos especiais e links simbólicos.
- CLI incluído no pacote; verificação de atualizações estáveis a cada 24 horas, ao manter o app aberto. Atualizações aguardam o fim das alterações de arquivos e interrompem consultas de leitura, que podem ser refeitas.
- Downloads restritos ao domínio oficial, verificação SHA-512, teste de versão antes da troca atômica e preservação do binário anterior.
- Interface em português, inglês e espanhol, com troca imediata em Configurações → Idioma, preferência persistida, datas e números locais, pluralização, mensagens da fila, sincronização e menu da bandeja traduzidos. Acesso por teclado e suporte a movimento reduzido.

## Comportamento de conflitos e limites

**Exclusões são desativadas por padrão e configuráveis por pareamento de arquivos.** Com a opção **Propagar exclusões**, remover um arquivo/pasta sincronizado no PC move a cópia remota para a lixeira Proton; remover no Drive move a cópia local para `.proton-drive-desktop/trash/<id>/`, junto com `origin.json`. Só ocorre se a cópia sobrevivente e seus descendentes não mudaram desde a última sincronização. Erros de listagem, conflitos, novos descendentes ou itens ignorados impedem a remoção. O aplicativo nunca usa exclusão permanente.

Mover/renomear arquivos é tratado como um novo caminho: a transferência é verificada antes de enviar o caminho antigo para a lixeira. Não preserva a identidade do nó nem os links compartilhados. Com a propagação desativada, o caminho antigo fica preservado para revisão. Exclusões ficam adiadas se o ciclo apresentar conflitos.

Se os dois lados mudarem, o app mantém as duas cópias. Compare-as e coloque o conteúdo desejado nos dois lados; quando os conteúdos coincidirem, a próxima execução estabelece o histórico novamente. Na primeira execução, arquivos com o mesmo conteúdo são reconhecidos pelo digest SHA-1 disponibilizado pelo SDK; arquivos diferentes com o mesmo nome geram conflito.

As cópias locais anteriores ficam em `.proton-drive-desktop/backups/<id>/` dentro da pasta sincronizada, com `origin.json` indicando o caminho original. Essa pasta nunca é enviada. Use **Opções da pasta → Abrir recuperação** para localizar as cópias e o caminho original registrado em `origin.json`. As cópias são mantidas até serem removidas manualmente; considere seu espaço em disco.

Também são ignorados `.git`, `node_modules` e `.DS_Store`. Os limites atuais são 100 níveis e 100.000 itens remotos por pareamento. As mudanças locais usam eventos nativos; mudanças feitas no Drive web ou em outro dispositivo são buscadas pelo intervalo da pasta, porque o CLI 0.8 não expõe um comando de assinatura de eventos remotos. O app precisa estar em execução.

Os 2 segundos são o período para agrupar mudanças antes de iniciar a comparação, não o tempo total da transferência. Há um único processo de conta do CLI por vez, para evitar concorrência na renovação de credenciais. O bloqueio é liberado entre chamadas: a indexação inteira não ocupa a fila de transferências. Mudanças recebidas durante uma transferência ficam pendentes. Escritas contínuas geram uma tentativa após no máximo 30 segundos, com as verificações de estabilidade do motor de sincronização. Falhas são repetidas com espera de 30 segundos. Não há montagem FUSE, modo offline sob demanda ou sincronização de pastas compartilhadas. PDFs/imagens são pré-visualizados até 20 MiB e texto até 2 MiB; arquivos maiores podem ser baixados normalmente.

**A sincronização não é 100% equivalente ao cliente oficial de outros sistemas.** O CLI 0.8 não exporta o conteúdo de Docs/Sheets; estes ficam online e não bloqueiam a sincronização dos arquivos comuns. Para obter uma cópia offline, exporte no editor Proton. Fotos novas podem ser enviadas, mas alterações em uma foto existente não podem substituir seu original pelo CLI: salve uma nova foto com outro nome. Exclusões de fotos, álbuns ou associações a álbuns não são propagadas; cópias locais são preservadas e sinalizadas para revisão. Renomear um álbum mantém os caminhos de fotos já baixadas para evitar duplicações, e novos itens seguem o nome atual.

A cópia por álbuns duplica no PC fotos também presentes na timeline ou em outros álbuns. O CLI não expõe miniaturas: a galeria mostra metadados, e os originais são baixados somente ao abrir a pré-visualização. Itens com nomes incompatíveis com caminhos Linux ou metadados incompletos interrompem a operação, preservando o histórico. As coleções expostas pelo CLI são o escopo sincronizado; mídias relacionadas a Live Photos dependem do que o CLI apresenta na listagem.

O CLI não oferece atualização condicional por revisão. O app confere as revisões imediatamente antes/depois das transferências e preserva versões, mas não pode eliminar uma corrida com outro dispositivo que edite o mesmo arquivo durante o envio. Evite executar outros processos do CLI simultaneamente com o aplicativo, pois os processos externos não compartilham o bloqueio interno.

## Desenvolver

Requisitos: Linux x86-64, Node.js 24 e Rust estável. O instalador usa o binário x64 **baseline**, compatível com CPUs sem AVX2.

No Fedora:

```sh
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config openssl-devel gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel libsecret
```

No Debian/Ubuntu:

```sh
sudo apt install build-essential libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libsecret-1-0
```

```sh
npm ci
npm run cli:install -- --locked # baixa o CLI fixado e verifica SHA-512
npm run computers:build    # requer Bun 1.3.14
npm run desktop           # aplicação Tauri + Nuxt
npm run dev               # somente prévia da interface, sem acesso ao computador
```

O modo navegador identifica explicitamente que é uma prévia. Ele não simula conta conectada nem transferências.

```sh
npm run typecheck
npm run test:ui           # testes dos componentes Vue
npm run test:browser      # cenário isolado em localhost:3001; somente dados fictícios
npm test                 # testes Rust e integrações com CLI temporário
npm run generate         # frontend estático
npm run tauri -- build --bundles appimage
npm run install:desktop  # instalação no perfil do usuário, sem sudo
```

Para verificar o atualizador real, baixando uma cópia oficial temporária:

```sh
cargo run -p drive-core --example verify_update
```

O download incorpora cerca de 112 MiB de CLI. O binário é ignorado pelo Git; `bin/release.json` registra a origem e o checksum. Execute `cli:install -- --locked` e `computers:build` antes de compilar um checkout novo. Os builds de distribuição suportam Linux x86-64. O CLI tem atualização automática no aplicativo. A partir de **0.1.2**, o AppImage inclui o ícone do CapyDock e o canal de atualizações das releases estáveis do GitHub, reconhecidos automaticamente ao importar no **Gear Lever**. A atualização da interface é gerenciada pelo Gear Lever, conforme as preferências dele; não é necessário configurar uma URL. Para versões anteriores, importe o AppImage novo uma vez.

## Arquitetura e dados

- `app/`: interface e comandos Tauri tipados.
- `src-tauri/`: aplicação nativa, bloqueio de operações, agendador, conta, preferências e janela web isolada.
- `crates/drive-core/`: protocolo do CLI, decisões de sincronização, monitor de arquivos/fila de alterações, transferência e atualizador.
- `scripts/install-cli.mjs`: obtenção do binário oficial no projeto.
- `scripts/install-desktop.mjs`: executável, CLI no PATH e atalho no perfil do usuário.

Configurações, histórico, snapshots e CLI atualizado ficam em `$XDG_DATA_HOME/io.github.protondrive.desktop/`, normalmente `~/.local/share/io.github.protondrive.desktop/`. Nenhuma senha é armazenada pelo aplicativo. O próprio CLI gerencia sua sessão pelo cofre do sistema.

A instalação local cria `~/.local/bin/proton-drive` apontando para o CLI gerenciado pelo app; assim, o comando no terminal recebe as mesmas atualizações. O binário de fábrica em `bin/` continua sendo a versão incluída no pacote.

## Validação e pendências externas

Os testes de integração usam arquivos temporários e um simulador do protocolo do CLI; não enviam arquivos de usuário. A validação com uma conta Proton real exige completar o login no aplicativo. A navegação web autenticada e transferências reais não fazem parte desses testes isolados.

O `npm audit` de 03/10/2026 registra alertas transitivos em `braces` e `node-forge`, usados pelas ferramentas de desenvolvimento/build do Nuxt. As versões publicadas consultadas ainda não contêm correção. O pacote desktop inclui apenas os assets estáticos e o backend Rust, sem servidor Node/Nitro. O servidor de desenvolvimento fica restrito a `127.0.0.1`; não o exponha publicamente.

## Fontes oficiais

- [Manifesto de versões do CLI](https://proton.me/download/drive/cli/version.json)
- [Downloads e checksums](https://proton.me/download/drive/cli/index.html)
- [Código e documentação do CLI](https://github.com/ProtonDriveApps/sdk/tree/main/cli)
- [Rotas oficiais do editor Docs/Sheets](https://github.com/ProtonMail/WebClients/blob/main/applications/drive/src/app/utils/docs/openInDocs.ts)
- [PDF.js](https://mozilla.github.io/pdf.js/)
- [Integração Nuxt e Tauri](https://v2.tauri.app/start/frontend/nuxt/)
- [DaisyUI com Nuxt](https://daisyui.com/docs/install/nuxt/)


### Computadores (Computers)

A versão 0.3.0 acessa a seção **Computers** real da conta, por `/devices`, além de Meus arquivos. Abra **Meu Drive → Computadores**, informe o nome do PC e clique em **Registrar PC**. Abra o computador e escolha **Adicionar pasta deste PC**; selecione uma pasta local e salve. O aplicativo cria a pasta correspondente dentro do computador no Drive. Também é possível escolher uma subpasta existente pelo explorador do diálogo de sincronização.

Os pareamentos em Computers usam os mesmos modos (bidirecional, enviar e receber), detecção local em cerca de 2 segundos, histórico, conflitos e exclusões opcionais dos demais pareamentos. O identificador do computador é salvo no pareamento e verificado antes de transferir; um computador removido/substituído ou nomes duplicados interrompem a operação. A coleção `/devices` e a raiz do computador não podem ser excluídas pelo sincronizador.

O registro usa `bin/proton-drive-computers`, um complemento deste projeto compilado a partir do SDK/CLI oficial, fixado no commit `5491f2eea473acaaa86b5969774b84610a37bd46` (CLI 0.8.0). Ele usa a sessão existente no cofre do sistema e identifica-se como `external-drive-desktop`; o único comando exposto é `device ensure <nome>`. Todas as transferências continuam usando o CLI oficial sem modificações, com sua atualização automática. O complemento é atualizado com o aplicativo; sua origem e SHA-512 estão em `bin/computers-release.json`.

Para recompilar o complemento Linux x64, instale Bun 1.3.14 e execute `npm run computers:build` antes do build Tauri. O script baixa o commit fixado e instala as dependências com o lockfile original. `PROTON_SDK_SOURCE` pode apontar para um checkout desse commit; `BUN_BINARY` permite indicar o executável Bun. A instalação verifica o hash do complemento e o inclui nos recursos do aplicativo, sem exigir Bun no computador de destino.

Pareamentos já existentes continuam em seu destino original. O aplicativo não move arquivos automaticamente entre Meus arquivos e Computers. Para usar a mesma pasta local em Computers, remova o pareamento antigo (essa ação preserva os arquivos) e configure o novo destino. A cópia antiga permanece em Meus arquivos; para evitar duplicar dados, mova a pasta pelo Proton Drive web antes de selecionar sua nova localização no aplicativo.

### Fila de operações (0.4.0)

A seção **Fila de operações** mostra todas as operações ativas, separadas em **Transferências e alterações**, **Navegação**, **Índice em segundo plano** e **Manutenção da sessão**, com detalhes e tempo decorrido. Solicitações manuais têm prioridade sobre automáticas ainda aguardando na mesma fila. Entre solicitações da mesma prioridade, vale a ordem de chegada.

- Navegação admite duas tarefas e indexação uma; alterações de arquivos continuam serializadas. Chamadas ao CLI se intercalam entre lotes/arquivos, com um processo autenticado por vez. Um arquivo grande em transferência ainda pode atrasar uma consulta nova; a biblioteca salva continua disponível.
- **Cancelar consulta** interrompe leituras e seu processo. **Parar após este arquivo** encerra a sincronização com segurança. Registro, configuração, autenticação e transferências iniciadas terminam normalmente. Login/logout e atualização do executável usam uma barreira exclusiva; leituras interrompidas por manutenção podem ser refeitas.
- **Pausar fila** suspende novas admissões, mantendo as atuais em execução. A pausa das pastas é independente. Os 30 resultados mais recentes ficam no histórico.
- Até 256 operações pendentes/ativas; a fila não persiste entre execuções. Eventos locais são agrupados, e a sincronização é retomada pela comparação dos arquivos. Logout cancela solicitações de conta ainda aguardando.

## Biblioteca rápida

Após verificar a conta, o app prepara Meus arquivos, Computadores, Álbuns e o índice de Documentos em segundo plano. A janela não aguarda uma varredura completa. Fotos e subpastas são lidas ao abrir. Na primeira indexação, os documentos aparecem conforme cada lote termina; nas seguintes, o cache salvo aparece imediatamente após a verificação da conta, enquanto necessário é atualizado.

O complemento `proton-drive-computers` adiciona `library list-batch`: até quatro pastas em paralelo na mesma sessão do SDK. Usa UIDs verificados para evitar resolver os mesmos caminhos ancestrais repetidamente. O bloqueio do processo é liberado após cada lote. Limites: 100 níveis e 100.000 itens; falhas preservam resultados anteriores e são exibidas com opção de tentar novamente.

`library-cache.json` contém metadados locais, não conteúdo de arquivos, e é privado ao usuário (0600), vinculado à identidade da conta. Não substitui os snapshots usados para decisões de sincronização. O cache suporta até 128 visualizações e 32 MiB em disco; renova listagens após 60 segundos e documentos após 5 minutos quando solicitado pelo agendador ou navegação. Mudanças feitas no app invalidam o cache. **Atualizar biblioteca** força uma nova consulta, sem duplicar uma em andamento. Cancelamento manual impede retomada automática daquela consulta nesta sessão; é possível retomá-la pelo botão Atualizar.

A gravação ocorre fora do bloqueio de leitura e salva resultados parciais a cada 30 segundos de indexação, além do término. A interface recebe apenas metadados de progresso quando a revisão da lista não mudou, mantém resultados disponíveis durante erros/atualizações, pagina em 48 itens e reutiliza a ordenação do `Intl.Collator`. A primeira varredura ainda depende da quantidade de pastas, da latência da API e da decifragem; largura de banda de 1 Gbps não elimina esses custos. Nenhuma taxa de transferência ou porcentagem fictícia é apresentada.

## Idiomas

English is the default and fallback language throughout the interface, native tray, dialogs and activity messages. New installations, settings without a language, and unsupported or null saved locales use English. Missing or empty translations also fall back to English. In **Settings → Language**, users can select **English**, **Português** or **Español**; an explicitly saved supported language is preserved. Changes apply immediately without interrupting the queue. Native preferences are saved in `settings.json`; the web preview uses local storage.

Os catálogos ficam em `app/i18n/{pt,en,es}.json`. O texto português é a chave estável, com parâmetros numerados (`{0}`, `{1}`). Datas, números e plurais usam `Intl`. Mensagens dinâmicas do Rust separam a chave dos parâmetros em um envelope identificado; nomes de arquivos, caminhos e conteúdo de documentos nunca são traduzidos. Mensagens novas do histórico acompanham o idioma escolhido; mensagens dinâmicas de versões antigas e diagnósticos externos do CLI/sistema conservam o texto original quando não há tradução conhecida. O conteúdo e o idioma do site Proton integrado são geridos pela própria Proton.

`npm run test:ui` verifica a paridade dos catálogos, parâmetros, textos e rótulos de acessibilidade, troca ao vivo, formatos locais, persistência e recuperação de falhas. `npm test` cobre compatibilidade das configurações antigas e a separação dos parâmetros nativos, além das regressões de sincronização.

Para medir apenas o cache local, sem acessar a conta ou transferir arquivos:

```bash
cargo run --manifest-path crates/drive-core/Cargo.toml --offline --example catalog_benchmark
```

O cenário sintético usa 10.000 documentos; `artifacts/catalog-benchmark.json` registra a execução de referência. Seus tempos não representam a velocidade de rede ou a primeira indexação de uma conta real.


## Recuperação automática de pasta local

Quando a raiz de um pareamento desaparece, o app recria a estrutura no armazenamento local verificado e restaura as cópias disponíveis no Drive, incluindo Fotos e Álbuns. Durante a recuperação, não envia arquivos nem propaga exclusões. As opções originais do pareamento voltam a valer após uma passagem completa sem conflitos; arquivos locais modificados durante a recuperação são preservados para revisão. Arquivos que nunca chegaram à nuvem não podem ser restaurados por esse mecanismo.

Um registro `<id>.local-root.json`, junto ao snapshot e fora da pasta sincronizada, guarda a localização/identidade do armazenamento e o estado da recuperação antes de criar diretórios. Uma interrupção, erro de download ou reinício mantém a recuperação ativa. Os snapshots anteriores são preservados. Pastas normais removidas *dentro* de uma raiz existente continuam seguindo a opção de propagação de exclusões configurada.

Para pareamentos anteriores sem esse registro e com raiz já ausente, a criação automática fica restrita à pasta pessoal ou a um volume atualmente montado e verificado. Pontos de montagem ausentes, identidade de armazenamento diferente, links simbólicos, arquivos ocupando o caminho e falta de permissões interrompem a tentativa sem substituir o local. O monitor detecta o retorno do caminho; falhas de sincronização automática são repetidas após 30 segundos. Não remonta discos, modifica permissões nem apaga arquivos para tentar reparar o local.

A atividade mostra “Recuperando pasta local” e, quando terminar, “Pasta local recuperada”. Registros antigos de falha permanecem no histórico da sessão.

## System tray

Em **Configurações → Aplicativo e sincronização → System tray**, ative a opção para que o **X** oculte a janela principal e mantenha a sincronização, o monitor de arquivos e a fila em segundo plano. A preferência fica salva e vale imediatamente. Por padrão, permanece desativada, inclusive em instalações anteriores.

O menu da bandeja acompanha o idioma do aplicativo e oferece **Abrir CapyDock**, **Sincronizar agora**, **Pausar/retomar sincronização**, **Abrir pasta local**, **Configurações** e **Sair**. O submenu de pastas lista os pareamentos atuais, inclusive os desativados. Sincronizar fica indisponível sem conexão, sem pastas ativas, durante a pausa ou quando já existe uma sincronização pendente. As ações usam a mesma fila e as mesmas preferências da interface; retomar também verifica as alterações acumuladas durante a pausa.

**Configurações** restaura a janela diretamente nessa tela, mesmo se o frontend ainda estiver iniciando. Abrir novamente pelo atalho restaura a mesma instância. As janelas auxiliares do Proton continuam fechando normalmente. O ícone da capivara é incorporado como **RGBA de 8 bits**, formato exigido pelo tray, e mantido em um diretório de cache exclusivo por execução. Isso também evita que outro processo remova seu PNG. Se a criação do ícone falhar, a opção fica indisponível e o X mantém o comportamento normal de fechamento. No Linux, a apresentação do ícone depende do suporte a AppIndicator da sessão desktop.

## Revisão da experiência

O PC passa a ter um vínculo persistido por conta, pelo UID do provedor. Repetir o registro ou mudar o nome informado reutiliza o computador vinculado. Uma intenção salva antes da criação permite retomar uma resposta incerta sem escolher outro nome. Para registros antigos, a interface oferece vincular um PC existente; não remove registros nem mescla arquivos automaticamente.

A biblioteca indica o computador atual, oferece ações de pastas no lugar do formulário repetido e leva à gestão quando a pasta já tem sincronização. “Configurar biblioteca” esclarece o escopo de Arquivos, Fotos e Álbuns. A revisão visual melhora contraste, tamanho de controles, navegação por teclado e estados acessíveis. O símbolo é uma capivara em SVG.

A [direção de produto](docs/product-direction.md) detalha a identidade CapyDock e a evolução para várias conexões de nuvem e instâncias Syncthing. Essas novas integrações ainda não estão implementadas.

## Identidade CapyDock

CapyDock é o nome escolhido para o aplicativo. A marca aparece na janela, navegação, bandeja, tela Sobre e atalho do Linux, com traduções em português, inglês e espanhol. O comando `capydock` abre o app; `proton-drive-desktop` continua disponível por compatibilidade. Proton Drive continua identificado como o serviço conectado.

O identificador Tauri e os caminhos de instalação e dados continuam os mesmos. O GTK usa esse identificador para associar a janela ao nome e ícone do CapyDock no GNOME/Wayland. A inicialização automática migra de “Proton Drive Desktop” para “CapyDock”, preservando opções existentes e usando o executável atual; uma configuração CapyDock já existente tem prioridade. Sessão, pareamentos, histórico e recuperação permanecem disponíveis.
