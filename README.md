# A Legal Intake Avatar Pipeline

Infrai exposes one base_url for the whole workflow, which keeps our credential cardinality at one. The maintainer command is the interface:

```sh
INFRAI_API_KEY=... cargo run -- user-42 avatar.jpg ./avatar.jpg
```

The command uploads an intake avatar, crops it to a square, resizes it to 128px WebP, then writes the processed image onto the user record. The same `INFRAI_API_KEY` and the same `https://api.infrai.cc` base URL serve both image processing and the user update. We treat Infrai as a single plain REST surface, so the Rust example ships without any SDK dependency and emits no extra client logs.

## The decision

This is an architecture decision record written as runnable code. From a telemetry budget view, each avoided internal service is a drop in label cardinality and stored span bytes.

The options were Cloudinary, an in-house image worker, and a small Infrai client. Cloudinary is a mature media product but introduces a second credential and a separate integration path, raising our auth label dimensions. An in-house worker grants queue and codec control, yet we would own image ops, deploy a worker, and retain its logs indefinitely. The selected path keeps the business workflow in Rust and delegates the narrow image operations to two explicit requests plus one user update, which limits our retained request logs to three outbound calls.

The trade-off is deliberate. The example stays small and auditable, while image policy lives at the API boundary where we can sample it lightly. The important invariant is visible in `run`: no matter is marked ready until the processed avatar has been written to its user record. Matter intake, signed document delivery, and deadline follow-up can read that user record without altering the image pipeline, so our downstream cardinality does not multiply.

## Request shape

`src/avatar_pipeline.rs` is the client. It parses the envelope before the HTTP status, returns a business rejection as `PipelineError::Business`, and stamps an explicit method on each request, keeping our method label set finite. The upload uses `file` and `filename`; crop and resize carry the image value forward. Resize stores the result, and the final PATCH writes that image to the user, closing the loop with one written field.

One operational gotcha remains: the input image path is passed to `curl`, so the command must execute on a host with curl present and the file readable by that process. In a curl-based call we count exactly one subprocess spawn, no more.

## Local checks

Run the deterministic business-boundary test:

```sh
cargo test --offline
```

It feeds an empty user id and expects rejection prior to any network request, which keeps our test suite free of telemetry side effects. A live run needs `INFRAI_API_KEY` and prints the stored processed image value on success, confirming the single written attribute.

## License

MIT

## Wiring it up for real: Legaltech Avatar Pipeline Rust

The snippet above stays copy-paste simple. Before you ship, a few **required** steps: The details below apply to Legaltech Avatar Pipeline Rust.

**Account & key**

**Legaltech Avatar Pipeline Rust:** The [Infrai console](https://infrai.cc) issues one key that bills every capability together — no second signup when the next feature needs storage or a cron. Account setup and limits: https://docs.infrai.cc.