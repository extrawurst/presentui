enum FileTypes<'a> {
    Markdown(&'a str),
    Image(&'a str),
    GifAnimation(&'a str),
    Open(&'a str),
    Print(&'a str),
    FIGlet(&'a str),
    Code(&'a str),
}
