# Bundled Postman sandbox libraries

These files are served to scripts through `require(name)`. They are vendored here, unchanged,
because scripts in shared collections expect them and the app must run offline. The version is
the one named in the URL they were fetched from; refresh them deliberately, and update this
file in the same change.

| Module | Version | Licence | Source |
|--------|---------|---------|--------|
| `lodash` | 4.17.21 | MIT | https://unpkg.com/lodash@4.17.21/lodash.min.js |
| `moment` | 2.30.1 | MIT | https://unpkg.com/moment@2.30.1/min/moment.min.js |
| `chai` | 4.5.0 | MIT | https://unpkg.com/chai@4.5.0/chai.js |

Each file keeps its own licence header; nothing is modified.
