# Builds e releases do CapyDock

A primeira release pública é **v0.1.0**. O único pacote distribuído é o **AppImage para Linux x86-64**. `SHA256SUMS` e `build-info.json` acompanham o pacote para conferir integridade e origem dos componentes incorporados.

## Fluxo automático

1. Faça commits em `dev` ou abra um pull request para `dev`. Use Conventional Commits no histórico final; em squash merges, o título do PR vira a mensagem relevante.
2. Cada push em `dev` executa checagem de versões, formatação, tipos, testes da interface, testes Rust, testes da automação de release, Clippy e compilação nativa. Não calcula uma nova versão, cria tags nem publica releases.
3. Para publicar, integre as mudanças de `dev` em `release` por merge ou pull request. O push em `release` repete as validações, calcula a próxima versão pelos commits desde a última tag e gera o AppImage no Ubuntu 22.04. O CLI Proton é fixado em `bin/release.json` e verificado por SHA-512; o complemento Computers é compilado do commit fixado.
4. O job de publicação recebe o artefato já validado. Cria um commit `chore(release): vX.Y.Z [skip ci]` com as versões e metadados dos binários incorporados, e avança somente `release` e a tag `vX.Y.Z` em um único push atômico, sem force push. A branch `dev` não é modificada pela publicação.
5. Uma release em rascunho recebe o AppImage, o checksum e a origem do build. Só se torna pública após concluir os uploads. As notas são geradas dos commits desde a tag anterior.

Pull requests apenas validam e compilam, inclusive quando o destino é `release`. Somente pushes ou execuções manuais na branch `release` podem publicar. Essa restrição existe tanto no workflow quanto no script de publicação. O job de build tem permissão de leitura; somente o job final tem `contents: write`. O token é o `GITHUB_TOKEN` do próprio workflow; não há PAT ou credencial Proton nos secrets.

Para publicar manualmente, use **Actions → CapyDock CI and release → Run workflow → release**. Selecionar `dev` executa apenas validação e compilação. O arquivo precisa existir na branch padrão para o botão manual ficar disponível. A publicação automática por push não depende desse botão.

## Regra de versão

| Commits desde a última tag                                                    | Resultado a partir de 0.1.0                       |
| ----------------------------------------------------------------------------- | ------------------------------------------------- |
| Primeira publicação, sem tags anteriores                                      | **0.1.0**, independentemente dos commits iniciais |
| `fix: ...` ou `perf: ...`                                                     | 0.1.1                                             |
| `feat: ...`                                                                   | 0.2.0                                             |
| `feat!: ...` ou footer `BREAKING CHANGE: ...`                                 | 1.0.0                                             |
| Somente `docs`, `chore`, `ci`, `test` ou refatoração sem mudança incompatível | Build no Actions, sem nova release                |

Vale o maior impacto encontrado no conjunto de commits. Tags fora do histórico da branch são ignoradas. O padrão é sempre `MAJOR.MINOR.PATCH`, sem reset de versão depois da primeira publicação. Não crie tags de release manualmente.

`package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, os dois manifests Cargo e `Cargo.lock` ficam com a mesma versão. `npm run version:check` verifica isso. As versões do CLI Proton e do complemento são independentes: não representam a versão do aplicativo.

## Trabalhar depois de uma publicação

O bot atualiza somente `release`. Depois de publicar, traga o commit de versão para `dev` por merge (ou por um PR de `release` para `dev`), preservando o histórico e a tag:

```sh
git switch dev
git pull --ff-only origin dev
git fetch origin --tags
git merge origin/release
git push origin dev
```

Esse push em `dev` não publica outra release. Para a próxima publicação, integre `dev` novamente em `release` sem descartar o histórico da última tag; evite squash ao sincronizar essas duas branches.

Se `release` avançar durante um build, esse build antigo não será publicado; a próxima execução inclui os commits pendentes. Mudanças simultâneas em `dev` não impedem a publicação e permanecem intactas. As execuções são serializadas por branch, e um upload em andamento não é cancelado automaticamente. Regras que proíbam o push fazem o workflow falhar, preservando o histórico. A automação não desativa proteções de branch.

## Falhas e retomada

Se o upload falhar depois de criar a tag, use **Re-run failed jobs** na mesma execução. O artefato fica retido por 14 dias. O job confere o SHA original e a tag, e retoma apenas o rascunho correspondente. Se os artefatos expirarem, repita todos os jobs da execução original para reconstruir a mesma versão. Uma release já publicada não tem seus arquivos substituídos; retomar uma versão antiga também não a promove sobre uma versão mais recente.

O AppImage não precisa de Node.js, Rust ou Bun instalados no computador de destino. A atualização automática existente continua sendo a do CLI Proton. Publicar AppImages no GitHub não instala atualizações da interface automaticamente.

Após o Tauri reunir as bibliotecas, `npm run release:appimage` restaura os executáveis incorporados e reempacota o AppDir com o plugin de saída AppImage. Isso preserva os checksums originais: o linuxdeploy altera o RPATH de executáveis ELF, incluindo recursos independentes do aplicativo. A preparação da release extrai os binários do AppImage final e confere os metadados e ambos os SHA-512 antes de disponibilizar o artefato. `CAPYDOCK_APPIMAGE_PLUGIN` permite indicar outro caminho para o plugin baixado pelo Tauri.

## Referências

- [Tauri: distribuição AppImage e escolha da base Linux](https://v2.tauri.app/distribute/appimage/)
- [GitHub: permissões e disparos de workflows pelo GITHUB_TOKEN](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow)
- [Analisador de Conventional Commits](https://github.com/semantic-release/commit-analyzer)
