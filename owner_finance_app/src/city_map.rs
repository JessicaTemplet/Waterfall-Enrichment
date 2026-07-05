/// Map common city names / abbreviations to their Craigslist subdomain.
/// Users can also type a subdomain directly (e.g. "sfbay", "newyork").
pub fn resolve_subdomain(input: &str) -> String {
    let key = input.trim().to_lowercase();
    let key = key.as_str();

    match key {
        // Texas
        "houston" | "hou"                          => "houston",
        "dallas" | "dfw"                           => "dallas",
        "austin" | "atx"                           => "austin",
        "san antonio" | "satx"                     => "sanantonio",
        "el paso"                                  => "elpaso",
        // California
        "los angeles" | "la"                       => "losangeles",
        "san francisco" | "sf" | "bay area"        => "sfbay",
        "san diego" | "sd"                         => "sandiego",
        "sacramento"                               => "sacramento",
        "fresno"                                   => "fresno",
        // New York
        "new york" | "nyc" | "new york city"       => "newyork",
        "buffalo"                                  => "buffalo",
        // Florida
        "miami"                                    => "miami",
        "orlando"                                  => "orlando",
        "tampa"                                    => "tampa",
        "jacksonville" | "jax"                     => "jacksonville",
        // Illinois
        "chicago"                                  => "chicago",
        // Pennsylvania
        "philadelphia" | "philly"                  => "philadelphia",
        "pittsburgh"                               => "pittsburgh",
        // Arizona
        "phoenix"                                  => "phoenix",
        "tucson"                                   => "tucson",
        // Georgia
        "atlanta"                                  => "atlanta",
        // Ohio
        "columbus"                                 => "columbus",
        "cleveland"                                => "cleveland",
        "cincinnati"                               => "cincinnati",
        // North Carolina
        "charlotte"                                => "charlotte",
        "raleigh"                                  => "raleigh",
        // Michigan
        "detroit"                                  => "detroit",
        // Washington
        "seattle"                                  => "seattle",
        // Colorado
        "denver"                                   => "denver",
        // Nevada
        "las vegas"                                => "lasvegas",
        // Tennessee
        "nashville"                                => "nashville",
        "memphis"                                  => "memphis",
        // Missouri
        "kansas city" | "kc"                       => "kansascity",
        "st louis" | "saint louis"                 => "stlouis",
        // Oregon
        "portland"                                 => "portland",
        // Indiana
        "indianapolis" | "indy"                    => "indianapolis",
        // Virginia
        "richmond"                                 => "richmond",
        "norfolk"                                  => "norfolk",
        // Minnesota
        "minneapolis"                              => "minneapolis",
        // Wisconsin
        "milwaukee"                                => "milwaukee",
        // Oklahoma
        "oklahoma city" | "okc"                    => "oklahomacity",
        // Louisiana
        "new orleans" | "nola"                     => "neworleans",
        // Maryland
        "baltimore"                                => "baltimore",
        // Alabama
        "birmingham"                               => "birmingham",
        // Kentucky
        "louisville"                               => "louisville",
        // South Carolina
        "columbia"                                 => "columbia",
        // Massachusetts
        "boston"                                   => "boston",
        // Arkansas
        "little rock"                              => "littlerock",
        // Utah
        "salt lake city" | "slc"                   => "saltlakecity",
        // New Mexico
        "albuquerque"                              => "albuquerque",
        // Hawaii
        "honolulu"                                 => "honolulu",

        // Anything else is passed through as-is (user typed the subdomain directly)
        other => return other.replace(' ', ""),
    }
    .to_string()
}
