# mule-project-basic

A small, valid MuleSoft 4 application used by RunnerGuard tests and examples.

## Layout

```
mule-project-basic/
├── pom.xml
├── mule-artifact.json
└── src/
    └── main/
        ├── mule/
        │   ├── global.xml        # listener-config + global error-handler
        │   └── order-api.xml     # flow + sub-flow
        └── resources/
            └── app.properties
```

## Expected findings under `rules/basic.json`

Two warnings (size of the flow is fine, names are kebab-case), no errors.
The `required-files-exist` rule passes because all three required files
are present. The `http:listener-connection` uses a property placeholder,
and the logger message contains no secret literals.
