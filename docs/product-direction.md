# CapyDock — identidade e direção de produto

Nome escolhido em 3 de outubro de 2026: **CapyDock**. A identidade está aplicada ao aplicativo, à bandeja e ao atalho do Linux na versão 0.1.0. A evolução para outros serviços descrita abaixo continua sendo um plano de produto.

## Marca

**Nome: CapyDock.** “Dock” sugere um ponto de encontro para arquivos, contas e dispositivos. Continua fazendo sentido com serviços de nuvem e conexões locais. Assinatura: **Seus arquivos. Em boa companhia.**

Logo: capivara de perfil, sem cauda, focinho largo e orelha pequena. Silhueta simples e poucos detalhes para funcionar em 24–32 px. Verde profundo e fundo mineral claro dão identidade própria; o desenho não depende de nuvem, cadeado ou marca de fornecedor. No tamanho de 16 px, usar a silhueta como base e preservar a leitura do animal, sem detalhes decorativos. O SVG é a fonte dos PNGs; não manter cópias redesenhadas.

## Continuidade da instalação

A marca de exibição muda; `io.github.protondrive.desktop`, o executável interno, os caminhos de dados/recuperação e a chave legada de inicialização automática permanecem estáveis. O novo comando `capydock` é um alias para a mesma instalação. Credenciais, preferências, vínculo do PC e sincronizações existentes são reutilizados.

## Revisão aplicada

| Problema | Comportamento revisado |
| --- | --- |
| Registrar novamente com outro nome criava outro PC | Vínculo persistido pelo UID do provedor, por conta nesta instalação; novos pedidos reutilizam o UID. |
| Resultado incerto da criação podia provocar nova tentativa com outro nome | A intenção é salva antes da chamada; o nome inicial é reaproveitado até confirmar o UID. |
| PC já registrado continuava exibindo formulário | Cartão de estado com “Ver pastas” e “Adicionar pasta deste PC”. |
| Registros anteriores não tinham vínculo local | Seletor explícito de PC existente; não escolher outro PC silenciosamente. |
| Nome remoto alterado | Identificar por UID e apresentar o nome/path retornado pelo provedor. |
| Registro removido na nuvem | Apresentar indisponibilidade; não recriar automaticamente ou apagar registros antigos. |
| “Sincronizar tudo” prometia mais do que entregava | “Configurar biblioteca”, explicando Arquivos, Fotos, Álbuns e as exceções de Computadores e documentos online. |
| Pasta já configurada oferecia novo pareamento | “Ver sincronização” leva à gestão existente. |
| Textos e controles pequenos e de baixo contraste | Foregrounds compartilhados, descrições legíveis, foco visível e controles principais de 40 px. |
| Navegação por teclado pouco clara | Link de pular para conteúdo, foco após navegação, estado acessível dos filtros, breadcrumbs e tabela com cabeçalhos. |
| App visualmente confundido com o provedor | Marca CapyDock, símbolo próprio de capivara e navegação “Biblioteca”. |

As contas e operações existentes não são alteradas pelos testes. Registros antigos duplicados não são mesclados ou removidos: não há evidência suficiente para decidir qual contém os arquivos corretos.

## Modelo para vários serviços e contas

O aplicativo deve separar cinco conceitos:

- **Provedor**: implementação Proton Drive, Google Drive, Dropbox ou Syncthing e suas capacidades.
- **Conexão**: uma conta ou instância específica, com ID próprio, nome editável e autenticação isolada. Duas contas Google são duas conexões, mesmo que usem o mesmo provedor.
- **Dispositivo**: este computador e outros dispositivos conhecidos. O ID local é estável; IDs remotos pertencem a cada conexão e não substituem a identidade local.
- **Local de arquivos**: uma raiz local, pasta de uma conta de nuvem ou pasta compartilhada por uma instância Syncthing.
- **Sincronização**: vínculo entre locais, direção, política de conflitos/exclusões, estado e fila. Todo caminho remoto deve ser interpretado junto com o ID de sua conexão.

O Syncthing precisa de um adaptador de instância/dispositivos/pastas compartilhadas. Não se deve forçar nele login OAuth, uma biblioteca de fotos ou conceitos de “Computers” específicos do Proton. A interface mostra apenas as capacidades suportadas: navegar, criar pasta, sincronizar, documentos online, fotos, álbuns e quotas.

## Navegação proposta

1. **Visão geral** — saúde das conexões, sincronizações ativas e problemas que exigem ação.
2. **Conexões** — conectar, reautenticar, nomear e desconectar cada conta/instância. Mostrar provedor e apelido em todos os contextos relevantes.
3. **Arquivos** — escolher conexão e navegar; fotos/documentos/álbuns aparecem quando o provedor oferece esse recurso.
4. **Sincronizações** — locais, destino identificado pela conexão, direção e comportamento de exclusões.
5. **Atividade** — transferências em execução, espera e histórico filtráveis por conexão; priorizar ações e progresso compreensível.
6. **Configurações** — idioma, bandeja, inicialização e atualizações do app. Opções particulares de um conector ficam dentro dele.

Não adicionar botões ativos de Google Drive/Dropbox/Syncthing antes de implementar e validar seus adaptadores.

## Sequência técnica proposta

1. Introduzir `connections` com IDs independentes, nomes de exibição e `providerId`; migrar a conta Proton existente preservando pares, UID de computador, cache, snapshots e política de exclusões.
2. Separar sessão, credenciais, cache, CLI/configuração e limites de concorrência por conexão. A integração atual do app usa uma única sessão do CLI: uma lista de contas na UI sozinha não oferece isolamento. Não criar várias instâncias até provar esse isolamento no motor.
3. Associar `connectionId` aos pares, tarefas, pré-visualizações, catálogo e binding do PC. Nunca deduplicar apenas por caminho ou endereço de email.
4. Extrair contrato de provedor a partir das operações reais existentes e capacidades; manter o scheduler e as proteções de arquivos compartilhados.
5. Implementar um novo provedor completo por vez. Testar a coexistência de duas contas do mesmo serviço, reconexão, perda de rede, rate limits e operações simultâneas antes de anunciar suporte.
6. Integrar Syncthing com suas próprias capacidades e estados; validar funcionamento em rede local e indisponibilidade de outros dispositivos.

## Critérios de aceitação

- Um clique repetido ou um nome alterado não cria outro registro para o PC já vinculado.
- Um erro após criar o PC não perde a intenção nem abre caminho para criar outro com outro nome.
- Uma conta nunca recebe ações, cache ou credenciais de outra.
- Ao remover uma conexão, parar/agendar com segurança seus trabalhos e explicar o destino dos dados locais; não apagar arquivos como efeito oculto.
- Ícones têm nome acessível ou são decorativos; os controles funcionam por teclado; erros não dependem só de cor.
- Testar com leitor de tela e zoom de 200% na sessão desktop real. As validações automatizadas e a prévia web não substituem isso.
