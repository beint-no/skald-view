`unsupported-cmyk.pdf` is a valid, fictional Skald PDF 2.0 document whose page
content uses an out-of-profile CMYK operator. Opening metadata succeeds and
rendering must fail explicitly. It exercises the embedding error state without
publishing a customer document.

Generated with Skald PDF 1.21.0 and the repository fixture generator's
`unsupported` subcommand (`GenerateFixtures.java tests/pdfs unsupported`, with
the same Skald module classpath as the ordinary fixture generator). `qpdf --check`
reports no syntax or stream encoding errors.
