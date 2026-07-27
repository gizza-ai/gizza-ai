## About this tool

AWS Signature Version 4 turns a request into a deterministic HMAC-SHA256 signature. This tool builds the same pieces AWS verifies on the server side: the canonical request, string to sign, credential scope, hexadecimal signature, Authorization header, and optional cURL command.

Use it when you are debugging S3, IAM, API Gateway, DynamoDB, or another AWS API call and need to compare each SigV4 step against your application. Enter the full request URL, region, service code, access key ID, secret key, and any headers or payload you will send. Leave the timestamp blank for the current UTC time, or provide a fixed `YYYYMMDDTHHMMSSZ` value to reproduce a known AWS documentation vector.

The calculation runs locally in the page. The secret key is used only to derive the signing key and is never printed in the result. For temporary credentials, paste the STS session token so the tool includes and signs `x-amz-security-token`.

Example: sign the AWS IAM ListUsers request with the documentation credentials, `GET`, region `us-east-1`, service `iam`, URL `https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08`, header `content-type: application/x-www-form-urlencoded; charset=utf-8`, and timestamp `20150830T123600Z`. Choosing `signature` returns `5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7`.

Limitations: this is header-based SigV4 (`AWS4-HMAC-SHA256`). It does not create presigned URLs or SigV4a ECDSA signatures, and it does not send the request for you.

## FAQ

<details>
<summary>Does this send my AWS secret key anywhere?</summary>

No. The page computes the HMAC-SHA256 signing key locally and never includes the secret key in the output. The Authorization header contains the access key ID, date scope, signed header list, and signature, but not the secret key.

</details>

<details>
<summary>What should I put in the service field?</summary>

Use the AWS service signing name, such as `s3`, `iam`, `execute-api`, `dynamodb`, or `ec2`. The service string becomes part of the credential scope and signing key, so it must match the API endpoint you are calling.

</details>

<details>
<summary>When should I enable unsigned payload or x-amz-content-sha256?</summary>

`unsigned_payload` signs the literal `UNSIGNED-PAYLOAD`, which is common for S3 streaming or browser-style uploads. `sign_content_sha256` adds and signs the `x-amz-content-sha256` header; S3 often requires it. For most JSON API calls, leave both off and sign the actual payload hash.

</details>

<details>
<summary>Can this make a presigned S3 URL?</summary>

No. Presigned URLs use query-string authentication with additional parameters and expiration handling. This tool focuses on header-based SigV4 requests and exposes the canonical request/string-to-sign so you can debug normal Authorization-header signing.

</details>
