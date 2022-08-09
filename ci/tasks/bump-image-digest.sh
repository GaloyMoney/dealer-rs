#!/bin/bash

set -eu

export digest=$(cat ./latest-image/digest)
export ref=$(cat ./repo/.git/short_ref)
export app_version=$(cat version/version)

pushd charts-repo

yq -i e '.stablesats.image.digest = strenv(digest)' ./charts/${CHARTS_SUBDIR}/values.yaml
yq -i e '.stablesats.image.git_ref = strenv(ref)' ./charts/${CHARTS_SUBDIR}/values.yaml
yq -i e '.stablesats.appVersion = strenv(app_version)' ./charts/${CHARTS_SUBDIR}/Chart.yaml

if [[ -z $(git config --global user.email) ]]; then
  git config --global user.email "bot@galoy.io"
fi
if [[ -z $(git config --global user.name) ]]; then
  git config --global user.name "CI Bot"
fi

(
  cd $(git rev-parse --show-toplevel)
  git merge --no-edit ${BRANCH}
  git add -A
  git status
  git commit -m "chore(${CHARTS_SUBDIR}): bump ${CHARTS_SUBDIR} image to '${digest}'"
)
