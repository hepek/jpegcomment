jpegcomment
===========

Tool for jpeg APP* and comment segment manipulation.


    jpegcomment 

    USAGE:
        jpegcomment [OPTIONS] --input <INPUT>

    OPTIONS:
        -i, --input <INPUT>        Input jpeg file
        -o, --output <OUTPUT>      Output file (defaults to stdout) [default: -]
        -c, --comment <COMMENT>    Set jpeg comment
        -d                         Delete jpeg comment
        -p                         Print jpeg comment
        -a, --anonymize            Delete all APP segments removing all image metadata
            --dbgprint             Print jpeg structure
        -h, --help                 Print help information
