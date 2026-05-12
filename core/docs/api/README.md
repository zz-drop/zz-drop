# zz-drop HTTP API v1

Public API contract for `zz-drop.net` and compatible self-hosted servers.

Base:

```text
/api/v1
```

OpenAPI location in public repo:

```text
core/docs/api/openapi.yaml
```

## Endpoints

- `GET /api/v1/info`
- `POST /api/v1/auth/register`
- `POST /api/v1/auth/login`
- `GET /api/v1/profiles`
- `POST /api/v1/profiles`
- `GET /api/v1/profiles/{alias}/blob`
- `PUT /api/v1/profiles/{alias}/blob?expected_version=N`
- `DELETE /api/v1/profiles/{alias}`
- `GET /api/v1/account/email-preferences`
- `PUT /api/v1/account/email-preferences`

## Error model

```json
{
  "error": "unauthorized",
  "message": "unauthorized"
}
```

Errors:

- `invalid_request`
- `unauthorized`
- `forbidden`
- `not_found`
- `version_conflict`
- `blob_too_large`
- `rate_limited`
- `server_error`

## Privacy

API never exposes provider metadata or decrypted profile data.

Server stores encrypted `profile.zz` blobs only.
