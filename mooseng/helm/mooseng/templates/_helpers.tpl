{{/*
Expand the name of the chart.
*/}}
{{- define "mooseng.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "mooseng.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "mooseng.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "mooseng.labels" -}}
helm.sh/chart: {{ include "mooseng.chart" . }}
{{ include "mooseng.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "mooseng.selectorLabels" -}}
app.kubernetes.io/name: {{ include "mooseng.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "mooseng.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "mooseng.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Master service name
*/}}
{{- define "mooseng.master.serviceName" -}}
{{- printf "%s-master" (include "mooseng.fullname" .) }}
{{- end }}

{{/*
Master headless service name
*/}}
{{- define "mooseng.master.headlessServiceName" -}}
{{- printf "%s-master-headless" (include "mooseng.fullname" .) }}
{{- end }}

{{/*
Chunkserver service name
*/}}
{{- define "mooseng.chunkserver.serviceName" -}}
{{- printf "%s-chunkserver" (include "mooseng.fullname" .) }}
{{- end }}

{{/*
Metalogger service name
*/}}
{{- define "mooseng.metalogger.serviceName" -}}
{{- printf "%s-metalogger" (include "mooseng.fullname" .) }}
{{- end }}