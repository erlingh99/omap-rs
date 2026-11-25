//! OCAD's grid and zone codes for coordinate reference systems with an EPSG
//! code, as Mapper lists them in `ocd_georef_fields.cpp`.
//!
//! A code is the grid number times 1000 plus the zone. UTM (grid 2) and
//! German Gauss-Kruger (grid 8) are handled apart from this table. Mapper's
//! rows for ESRI codes, which are not EPSG codes and do not fit one, are left
//! out.

/// The grid number, zone and EPSG code of each known grid and zone.
pub(in crate::ocd) const GRID_ZONE_EPSG: [(u16, u16, u16); 524] = [
    (35, 0, 2462),   // Albanian 1987 / Gauss-Kruger zone 4
    (20, 1, 28348),  // GDA94 / MGA zone 48
    (20, 2, 28349),  // GDA94 / MGA zone 49
    (20, 3, 28350),  // GDA94 / MGA zone 50
    (20, 4, 28351),  // GDA94 / MGA zone 51
    (20, 5, 28352),  // GDA94 / MGA zone 52
    (20, 6, 28353),  // GDA94 / MGA zone 53
    (20, 7, 28354),  // GDA94 / MGA zone 54
    (20, 8, 28355),  // GDA94 / MGA zone 55
    (20, 9, 28356),  // GDA94 / MGA zone 56
    (20, 10, 28357), // GDA94 / MGA zone 57
    (20, 11, 28358), // GDA94 / MGA zone 58
    (20, 12, 3112),  // GDA94 / Geoscience Australia Lambert
    (20, 16, 7849),  // GDA2020 / MGA zone 49
    (20, 17, 7850),  // GDA2020 / MGA zone 50
    (20, 18, 7851),  // GDA2020 / MGA zone 51
    (20, 19, 7852),  // GDA2020 / MGA zone 52
    (20, 20, 7853),  // GDA2020 / MGA zone 53
    (20, 21, 7854),  // GDA2020 / MGA zone 54
    (20, 22, 7855),  // GDA2020 / MGA zone 55
    (20, 23, 7856),  // GDA2020 / MGA zone 56
    (3, 28, 31257),  // MGI / Austria GK M28
    (3, 31, 31258),  // MGI / Austria GK M31
    (3, 34, 31259),  // MGI / Austria GK M34
    (3, 35, 31254),  // MGI / Austria GK West
    (3, 36, 31255),  // MGI / Austria GK Central
    (3, 38, 31256),  // MGI / Austria GK East
    (4, 1, 31300),   // Belge 1972 / Belge Lambert 72
    (4, 2, 3447),    // ETRS89 / Belgian Lambert 2005
    (4, 3, 3812),    // ETRS89 / Belgian Lambert 2008
    (52, 4, 22521),  // Corrego Alegre 1970-72 / UTM zone 21S
    (52, 5, 22522),  // Corrego Alegre 1970-72 / UTM zone 22S
    (52, 6, 22523),  // Corrego Alegre 1970-72 / UTM zone 23S
    (52, 7, 22524),  // Corrego Alegre 1970-72 / UTM zone 24S
    (52, 8, 22525),  // Corrego Alegre 1970-72 / UTM zone 25S
    (52, 9, 5875),   // SAD69(96) / UTM zone 18S
    (52, 10, 5876),  // SAD69(96) / UTM zone 19S
    (52, 11, 5877),  // SAD69(96) / UTM zone 20S
    (52, 12, 5531),  // SAD69(96) / UTM zone 21S
    (52, 13, 5858),  // SAD69(96) / UTM zone 22S
    (52, 14, 5533),  // SAD69(96) / UTM zone 23S
    (52, 15, 5534),  // SAD69(96) / UTM zone 24S
    (52, 16, 5535),  // SAD69(96) / UTM zone 25S
    (52, 17, 31978), // SIRGAS 2000 / UTM zone 18S
    (52, 18, 31979), // SIRGAS 2000 / UTM zone 19S
    (52, 19, 31980), // SIRGAS 2000 / UTM zone 20S
    (52, 20, 31981), // SIRGAS 2000 / UTM zone 21S
    (52, 21, 31982), // SIRGAS 2000 / UTM zone 22S
    (52, 22, 31983), // SIRGAS 2000 / UTM zone 23S
    (52, 23, 31984), // SIRGAS 2000 / UTM zone 24S
    (52, 24, 31985), // SIRGAS 2000 / UTM zone 25S
    (5, 0, 27700),   // OSGB 1936
    (70, 0, 2019),   // NAD27(76) / MTM zone 10
    (31, 7, 3765),   // HTRS96 / Croatia TM
    (46, 1, 5514),   // S-JTSK Krovak East North
    (46, 2, 28403),  // Pulkovo 1942 / Gauss-Kruger zone 3
    (27, 0, 3301),   // Estonian Coordinate System of 1997
    (6, 1, 2391),    // KKJ / Finland zone 1
    (6, 2, 2392),    // KKJ / Finland zone 2
    (6, 3, 2393),    // KKJ / Finland Uniform Coordinate System
    (6, 4, 2394),    // KKJ / Finland zone 4
    (6, 5, 3067),    // TM35FIN, ETRS89 / ETRS-TM35FIN
    (6, 6, 3873),    // ETRS89 / GK19FIN
    (6, 7, 3874),    // ETRS89 / GK20FIN
    (6, 8, 3875),    // ETRS89 / GK21FIN
    (6, 9, 3876),    // ETRS89 / GK22FIN
    (6, 10, 3877),   // ETRS89 / GK23FIN
    (6, 11, 3878),   // ETRS89 / GK24FIN
    (6, 12, 3879),   // ETRS89 / GK25FIN
    (6, 13, 3880),   // ETRS89 / GK26FIN
    (6, 14, 3881),   // ETRS89 / GK27FIN
    (6, 15, 3882),   // ETRS89 / GK28FIN
    (6, 16, 3883),   // ETRS89 / GK29FIN
    (6, 17, 3884),   // ETRS89 / GK30FIN
    (6, 18, 3885),   // ETRS89 / GK31FIN
    (6, 19, 3126),   // ETRS89 / ETRS-GK19FIN
    (6, 20, 3127),   // ETRS89 / ETRS-GK20FIN
    (6, 21, 3128),   // ETRS89 / ETRS-GK21FIN
    (6, 22, 3129),   // ETRS89 / ETRS-GK22FIN
    (6, 23, 3130),   // ETRS89 / ETRS-GK23FIN
    (6, 24, 3131),   // ETRS89 / ETRS-GK24FIN
    (6, 25, 3132),   // ETRS89 / ETRS-GK25FIN
    (6, 26, 3133),   // ETRS89 / ETRS-GK26FIN
    (6, 27, 3134),   // ETRS89 / ETRS-GK27FIN
    (6, 28, 3135),   // ETRS89 / ETRS-GK28FIN
    (6, 29, 3136),   // ETRS89 / ETRS-GK29FIN
    (6, 30, 3137),   // ETRS89 / ETRS-GK30FIN
    (6, 31, 3138),   // ETRS89 / ETRS-GK31FIN
    (6, 32, 3387),   // KKJ / Finland zone 5
    (7, 1, 27591),   // NTF_Paris_Nord_France
    (7, 2, 27581),   // NTF_Paris_France_I
    (7, 3, 27592),   // NTF_Paris_Centre_France
    (7, 4, 27572),   // NTF (Paris) / Lambert zone II
    (7, 5, 27593),   // NTF_Paris_Sud_France
    (7, 6, 27583),   // NTF_Paris_France_III
    (7, 7, 27594),   // NTF_Paris_Corse
    (7, 8, 27584),   // NTF_Paris_France_IV
    (7, 9, 2154),    // RGF93 / Lambert-93
    (7, 10, 3942),   // RGF93 / CC42
    (7, 11, 3943),   // RGF93 / CC43
    (7, 12, 3944),   // RGF93 / CC44
    (7, 13, 3945),   // RGF93 / CC45
    (7, 14, 3946),   // RGF93 / CC46
    (7, 15, 3947),   // RGF93 / CC47
    (7, 16, 3948),   // RGF93 / CC48
    (7, 17, 3949),   // RGF93 / CC49
    (7, 18, 3950),   // RGF93 / CC50
    (8, 6, 25831),   // ETRS89 / UTM zone 31N
    (8, 7, 25832),   // ETRS89 / UTM zone 32N
    (8, 8, 25833),   // ETRS89 / UTM zone 33N
    (8, 13, 3068),   // DHDN / Soldner Berlin
    (8, 14, 3068),   // DHDN / Soldner Berlin
    (29, 0, 2100),   // GGRS87 / Greek Grid
    (61, 0, 2326),   // Hong Kong 1980 Grid System
    (49, 0, 23700),  // HD72 / EOV
    (38, 4, 3057),   // ISN93 / Lambert 1993
    (17, 1, 2206),   // ED50 / 3-degree Gauss-Kruger zone 9
    (17, 2, 2207),   // ED50 / 3-degree Gauss-Kruger zone 10
    (17, 3, 2208),   // ED50 / 3-degree Gauss-Kruger zone 11
    (17, 4, 2209),   // ED50 / 3-degree Gauss-Kruger zone 12
    (17, 5, 2210),   // ED50 / 3-degree Gauss-Kruger zone 13
    (17, 6, 2211),   // ED50 / 3-degree Gauss-Kruger zone 14
    (17, 7, 2212),   // ED50 / 3-degree Gauss-Kruger zone 15
    (17, 8, 7006),   // Nahrwan 1934 / UTM zone 38N
    (9, 1, 29902),   // TM65 / Irish Grid
    (9, 2, 29903),   // TM75 / Irish Grid
    (9, 3, 2157),    // IRENET95 / Irish Transverse Mercator
    (34, 0, 29902),  // TM65 / Irish Grid
    (53, 0, 2039),   // Israel 1993 / Israeli TM Grid
    (16, 1, 4685),
    (16, 2, 4686),   // MAGNA-SIRGAS
    (16, 3, 23031),  // ED50 / UTM zone 31N
    (16, 4, 23032),  // ED50 / UTM zone 32N
    (16, 5, 23033),  // ED50 / UTM zone 33N
    (16, 6, 23034),  // ED50 / UTM zone 34N
    (16, 7, 25832),  // ETRS89 / UTM zone 32N
    (16, 8, 25833),  // ETRS89 / UTM zone 33N
    (16, 9, 6707),   // RDN2008 / UTM zone 32N (N-E)
    (16, 10, 6708),  // RDN2008 / UTM zone 33N (N-E)
    (16, 11, 6875),  // RDN2008 / Italy zone (N-E)
    (16, 12, 6876),  // RDN2008 / Zone 12 (N-E)
    (57, 0, 2041),   // Abidjan 1987 / UTM zone 30N
    (11, 1, 30161),  // Tokyo / Japan Plane Rectangular CS I
    (11, 2, 30162),  // Tokyo / Japan Plane Rectangular CS II
    (11, 3, 30163),  // Tokyo / Japan Plane Rectangular CS III
    (11, 4, 30164),  // Tokyo / Japan Plane Rectangular CS IV
    (11, 5, 30165),  // Tokyo / Japan Plane Rectangular CS V
    (11, 6, 30166),  // Tokyo / Japan Plane Rectangular CS VI
    (11, 7, 30167),  // Tokyo / Japan Plane Rectangular CS VII
    (11, 8, 30168),  // Tokyo / Japan Plane Rectangular CS VIII
    (11, 9, 30169),  // Tokyo / Japan Plane Rectangular CS IX
    (11, 10, 30170), // Tokyo / Japan Plane Rectangular CS X
    (11, 11, 30171), // Tokyo / Japan Plane Rectangular CS XI
    (11, 12, 30172), // Tokyo / Japan Plane Rectangular CS XII
    (11, 13, 30173), // Tokyo / Japan Plane Rectangular CS XIII
    (11, 14, 30174), // Tokyo / Japan Plane Rectangular CS XIV
    (11, 15, 30175), // Tokyo / Japan Plane Rectangular CS XV
    (11, 16, 30176), // Tokyo / Japan Plane Rectangular CS XVI
    (11, 17, 30177), // Tokyo / Japan Plane Rectangular CS XVII
    (11, 18, 30178), // Tokyo / Japan Plane Rectangular CS XVIII
    (11, 19, 30179), // Tokyo / Japan Plane Rectangular CS XIX
    (11, 20, 2443),  // JGD2000 / Japan Plane Rectangular CS I
    (11, 21, 2444),  // JGD2000 / Japan Plane Rectangular CS II
    (11, 22, 2445),  // JGD2000 / Japan Plane Rectangular CS III
    (11, 23, 2446),  // JGD2000 / Japan Plane Rectangular CS IV
    (11, 24, 2447),  // JGD2000 / Japan Plane Rectangular CS V
    (11, 25, 2448),  // JGD2000 / Japan Plane Rectangular CS VI
    (11, 26, 2449),  // JGD2000 / Japan Plane Rectangular CS VII
    (11, 27, 2450),  // JGD2000 / Japan Plane Rectangular CS VIII
    (11, 28, 2451),  // JGD2000 / Japan Plane Rectangular CS IX
    (11, 29, 2452),  // JGD2000 / Japan Plane Rectangular CS X
    (11, 30, 2453),  // JGD2000 / Japan Plane Rectangular CS XI
    (11, 31, 2454),  // JGD2000 / Japan Plane Rectangular CS XII
    (11, 32, 2455),  // JGD2000 / Japan Plane Rectangular CS XIII
    (11, 33, 2456),  // JGD2000 / Japan Plane Rectangular CS XIV
    (11, 34, 2457),  // JGD2000 / Japan Plane Rectangular CS XV
    (11, 35, 2458),  // JGD2000 / Japan Plane Rectangular CS XVI
    (11, 36, 2459),  // JGD2000 / Japan Plane Rectangular CS XVII
    (11, 37, 2460),  // JGD2000 / Japan Plane Rectangular CS XVIII
    (11, 39, 2461),  // JGD2000 / Japan Plane Rectangular CS XIX
    (68, 0, 21037),  // Arc 1960 / UTM zone 37S
    (28, 0, 3059),   // LKS92 / Latvia TM
    (26, 0, 3346),   // LKS94 / Lithuania TM
    (33, 1, 2169),   // Luxembourg 1930 / Gauss
    (33, 2, 23031),  // ED50 / UTM zone 31N
    (54, 1, 3375),   // GDM2000 / Peninsula RSO
    (54, 2, 3376),   // GDM2000 / East Malaysia BRSO
    (41, 1, 28991),  // Amersfoort / RD Old
    (41, 2, 23031),  // ED50 / UTM zone 31N
    (41, 3, 23032),  // ED50 / UTM zone 32N
    (41, 4, 28992),  // Amersfoort / RD New
    (19, 0, 2193),   // NZGD2000 / New Zealand Transverse Mercator 2000
    (24, 0, 4272),   // NZGD49
    (12, 1, 27391),  // NGO 1948 (Oslo) / NGO zone I
    (12, 2, 27392),  // NGO 1948 (Oslo) / NGO zone II
    (12, 3, 27393),  // NGO 1948 (Oslo) / NGO zone III
    (12, 4, 27394),  // NGO 1948 (Oslo) / NGO zone IV
    (12, 5, 27395),  // NGO 1948 (Oslo) / NGO zone V
    (12, 6, 27396),  // NGO 1948 (Oslo) / NGO zone VI
    (12, 7, 27397),  // NGO 1948 (Oslo) / NGO zone VII
    (12, 8, 27398),  // NGO 1948 (Oslo) / NGO zone VIII
    (12, 9, 23031),  // ED50 / UTM zone 31N
    (12, 10, 23032), // ED50 / UTM zone 32N
    (12, 11, 23033), // ED50 / UTM zone 33N
    (12, 12, 23034), // ED50 / UTM zone 34N
    (12, 13, 23035), // ED50 / UTM zone 35N
    (12, 14, 23036), // ED50 / UTM zone 36N
    (48, 1, 2180),   // ETRS89 / Poland CS92
    (48, 2, 2176),   // ETRS89 / Poland CS2000 zone 5
    (48, 3, 2177),   // ETRS89 / Poland CS2000 zone 6
    (48, 4, 2178),   // ETRS89 / Poland CS2000 zone 7
    (48, 5, 2179),   // ETRS89 / Poland CS2000 zone 8
    (64, 0, 3785),
    (42, 1, 27493),   // Datum 73 / Modified Portuguese Grid
    (42, 2, 23028),   // ED50 / UTM zone 28N
    (69, 0, 2586),    // Pulkovo 1942 / 3-degree Gauss-Kruger CM 33E
    (47, 1, 5514),    // S-JTSK Krovak East North
    (47, 2, 28403),   // Pulkovo_1942_GK_Zone_3
    (47, 3, 28404),   // Pulkovo 1942 / Gauss-Kruger zone 4
    (15, 1, 3912),    // MGI 1901 / Slovene National Grid
    (15, 2, 3794),    // Slovenia 1996 / Slovene National Grid
    (18, 1, 2046),    // Hartebeesthoek94 / Lo15
    (18, 2, 2047),    // Hartebeesthoek94 / Lo17
    (18, 3, 2048),    // Hartebeesthoek94 / Lo19
    (18, 4, 2049),    // Hartebeesthoek94 / Lo21
    (18, 5, 2050),    // Hartebeesthoek94 / Lo23
    (18, 6, 2051),    // Hartebeesthoek94 / Lo25
    (18, 7, 2052),    // Hartebeesthoek94 / Lo27
    (18, 8, 2053),    // Hartebeesthoek94 / Lo29
    (18, 9, 2054),    // Hartebeesthoek94 / Lo31
    (18, 10, 2055),   // Hartebeesthoek94 / Lo33
    (13, 1, 6124),    // WGS 84 / EPSG Arctic zone 4-12
    (13, 2, 3006),    // SWEREF99 TM
    (13, 3, 3007),    // SWEREF99 12 00
    (13, 4, 3008),    // SWEREF99 13 30
    (13, 5, 3009),    // SWEREF99 15 00
    (13, 6, 3010),    // SWEREF99 16 30
    (13, 7, 3011),    // SWEREF99 18 00
    (13, 8, 3012),    // SWEREF99 14 15
    (13, 9, 3013),    // SWEREF99 15 45
    (13, 10, 3014),   // SWEREF99 17 15
    (13, 11, 3015),   // SWEREF99 18 45
    (13, 12, 3016),   // SWEREF99 20 15
    (13, 13, 3017),   // SWEREF99 21 45
    (13, 14, 3018),   // SWEREF99 23 15
    (14, 1, 21781),   // CH1903 / LV03 -- Swiss CH1903 / LV03
    (14, 2, 2056),    // CH1903+ / LV95 -- Swiss CH1903+ / LV95
    (21, 1, 23032),   // ED50 / UTM zone 32N
    (21, 2, 23033),   // ED50 / UTM zone 33N
    (21, 3, 25832),   // ETRS89 / UTM zone 32N
    (21, 4, 25833),   // ETRS89 / UTM zone 33N
    (30, 1, 23028),   // ED50 / UTM zone 28N
    (30, 2, 23029),   // ED50 / UTM zone 29N
    (30, 3, 23030),   // ED50 / UTM zone 30N
    (30, 4, 23030),   // ED50 / UTM zone 30N
    (30, 5, 23031),   // ED50 / UTM zone 31N
    (30, 6, 23031),   // ED50 / UTM zone 31N
    (32, 0, 23033),   // ED50 / UTM zone 33N
    (36, 1, 25833),   // ETRS89 / UTM zone 33N
    (36, 2, 25834),   // ETRS89 / UTM zone 34N
    (37, 1, 25834),   // ETRS89 / UTM zone 34N
    (37, 2, 25835),   // ETRS89 / UTM zone 35N
    (39, 0, 23033),   // ED50 / UTM zone 33N
    (40, 0, 23032),   // ED50 / UTM zone 32N
    (44, 0, 23033),   // ED50 / UTM zone 33N
    (50, 1, 26929),   // NAD83 / Alabama East
    (50, 2, 26930),   // NAD83 / Alabama West
    (50, 4, 26932),   // NAD83 / Alaska zone 2
    (50, 5, 26933),   // NAD83 / Alaska zone 3
    (50, 6, 26934),   // NAD83 / Alaska zone 4
    (50, 7, 26935),   // NAD83 / Alaska zone 5
    (50, 8, 26936),   // NAD83 / Alaska zone 6
    (50, 9, 26937),   // NAD83 / Alaska zone 7
    (50, 10, 26938),  // NAD83 / Alaska zone 8
    (50, 11, 26939),  // NAD83 / Alaska zone 9
    (50, 12, 26940),  // NAD83 / Alaska zone 10
    (50, 13, 26948),  // NAD83 / Arizona East
    (50, 14, 26949),  // NAD83 / Arizona Central
    (50, 15, 26950),  // NAD83 / Arizona West
    (50, 16, 26951),  // NAD83 / Arkansas North
    (50, 17, 26952),  // NAD83 / Arkansas South
    (50, 18, 26941),  // NAD83 / California zone 1
    (50, 19, 26942),  // NAD83 / California zone 2
    (50, 20, 26943),  // NAD83 / California zone 3
    (50, 21, 26944),  // NAD83 / California zone 4
    (50, 22, 26945),  // NAD83 / California zone 5
    (50, 23, 26946),  // NAD83 / California zone 6
    (50, 24, 26953),  // NAD83 / Colorado North
    (50, 25, 26954),  // NAD83 / Colorado Central
    (50, 26, 26955),  // NAD83 / Colorado South
    (50, 27, 26956),  // NAD83 / Connecticut
    (50, 28, 26957),  // NAD83 / Delaware
    (50, 29, 26958),  // NAD83 / Florida East
    (50, 30, 26959),  // NAD83 / Florida West
    (50, 31, 26960),  // NAD83 / Florida North
    (50, 32, 26966),  // NAD83 / Georgia East
    (50, 33, 26967),  // NAD83 / Georgia West
    (50, 34, 26961),  // NAD83 / Hawaii zone 1
    (50, 35, 26962),  // NAD83 / Hawaii zone 2
    (50, 36, 26963),  // NAD83 / Hawaii zone 3
    (50, 37, 26964),  // NAD83 / Hawaii zone 4
    (50, 38, 26965),  // NAD83 / Hawaii zone 5
    (50, 39, 26968),  // NAD83 / Idaho East
    (50, 40, 26969),  // NAD83 / Idaho Central
    (50, 41, 26970),  // NAD83 / Idaho West
    (50, 42, 26971),  // NAD83 / Illinois East
    (50, 43, 26972),  // NAD83 / Illinois West
    (50, 44, 26973),  // NAD83 / Indiana East
    (50, 45, 26974),  // NAD83 / Indiana West
    (50, 46, 26975),  // NAD83 / Iowa North
    (50, 47, 26976),  // NAD83 / Iowa South
    (50, 48, 26977),  // NAD83 / Kansas North
    (50, 49, 26978),  // NAD83 / Kansas South
    (50, 50, 26979),  // NAD_1983_StatePlane_Kentucky_North_FIPS_1601
    (50, 51, 26980),  // NAD83 / Kentucky South
    (50, 52, 26981),  // NAD83 / Louisiana North
    (50, 53, 26982),  // NAD83 / Louisiana South
    (50, 54, 32199),  // NAD83 / Louisiana Offshore
    (50, 55, 26983),  // NAD83 / Maine East
    (50, 56, 26984),  // NAD83 / Maine West
    (50, 57, 26985),  // NAD83 / Maryland
    (50, 58, 26986),  // NAD83 / Massachusetts Mainland
    (50, 59, 26987),  // NAD83 / Massachusetts Island
    (50, 60, 26988),  // NAD83 / Michigan North
    (50, 61, 26989),  // NAD83 / Michigan Central
    (50, 62, 26990),  // NAD83 / Michigan South
    (50, 63, 26991),  // NAD83 / Minnesota North
    (50, 64, 26992),  // NAD83 / Minnesota Central
    (50, 65, 26993),  // NAD83 / Minnesota South
    (50, 66, 26994),  // NAD83 / Mississippi East
    (50, 67, 26995),  // NAD83 / Mississippi West
    (50, 68, 26996),  // NAD83 / Missouri East
    (50, 69, 26997),  // NAD83 / Missouri Central
    (50, 70, 26998),  // NAD83 / Missouri West
    (50, 71, 32100),  // NAD83 / Montana
    (50, 72, 32104),  // NAD83 / Nebraska
    (50, 73, 32107),  // NAD83 / Nevada East
    (50, 74, 32108),  // NAD83 / Nevada Central
    (50, 75, 32109),  // NAD83 / Nevada West
    (50, 76, 32110),  // NAD83 / New Hampshire
    (50, 77, 32111),  // NAD83 / New Jersey
    (50, 78, 32112),  // NAD83 / New Mexico East
    (50, 79, 32113),  // NAD83 / New Mexico Central
    (50, 80, 32114),  // NAD83 / New Mexico West
    (50, 81, 32115),  // NAD83 / New York East
    (50, 82, 32116),  // NAD83 / New York Central
    (50, 83, 32117),  // NAD83 / New York West
    (50, 84, 32118),  // NAD83 / New York Long Island
    (50, 85, 32119),  // NAD83 / North Carolina
    (50, 86, 32120),  // NAD83 / North Dakota North
    (50, 87, 32121),  // NAD83 / North Dakota South
    (50, 88, 32122),  // NAD83 / Ohio North
    (50, 89, 32123),  // NAD83 / Ohio South
    (50, 90, 32124),  // NAD83 / Oklahoma North
    (50, 91, 32125),  // NAD83 / Oklahoma South
    (50, 92, 32126),  // NAD83 / Oregon North
    (50, 93, 32127),  // NAD83 / Oregon South
    (50, 94, 32128),  // NAD83 / Pennsylvania North
    (50, 95, 32129),  // NAD83 / Pennsylvania South
    (50, 96, 32130),  // NAD83 / Rhode Island
    (50, 97, 32133),  // NAD83 / South Carolina
    (50, 98, 32134),  // NAD83 / South Dakota North
    (50, 99, 32135),  // NAD83 / South Dakota South
    (50, 100, 32136), // NAD83 / Tennessee
    (50, 101, 32137), // NAD83 / Texas North
    (50, 102, 32138), // NAD83 / Texas North Central
    (50, 103, 32139), // NAD83 / Texas Central
    (50, 104, 32140), // NAD83 / Texas South Central
    (50, 105, 32141), // NAD83 / Texas South
    (50, 106, 32142), // NAD83 / Utah North
    (50, 107, 32143), // NAD83 / Utah Central
    (50, 108, 32144), // NAD83 / Utah South
    (50, 109, 32145), // NAD83 / Vermont
    (50, 110, 32146), // NAD83 / Virginia North
    (50, 111, 32147), // NAD83 / Virginia South
    (50, 112, 32148), // NAD83 / Washington North
    (50, 113, 32149), // NAD83 / Washington South
    (50, 114, 32150), // NAD83 / West Virginia North
    (50, 115, 32151), // NAD83 / West Virginia South
    (50, 116, 32152), // NAD83 / Wisconsin North
    (50, 117, 32153), // NAD83 / Wisconsin Central
    (50, 118, 32154), // NAD83 / Wisconsin South
    (50, 119, 32155), // NAD83 / Wyoming East
    (50, 120, 32156), // NAD83 / Wyoming East Central
    (50, 121, 32157), // NAD83 / Wyoming West Central
    (50, 122, 32158), // NAD83 / Wyoming West
    (50, 123, 32161), // NAD83 / Puerto Rico & Virgin Is.
    (66, 13, 2222),   // NAD83 / Arizona East (ft)
    (66, 14, 2223),   // NAD83 / Arizona Central (ft)
    (66, 15, 2224),   // NAD83 / Arizona West (ft)
    (66, 16, 3433),   // NAD83 / Arkansas North (ftUS)
    (66, 17, 3434),   // NAD83 / Arkansas South (ftUS)
    (66, 18, 2225),   // NAD83 / California zone 1 (ftUS)
    (66, 19, 2226),   // NAD83 / California zone 2 (ftUS)
    (66, 20, 2227),   // NAD83 / California zone 3 (ftUS)
    (66, 21, 2228),   // NAD83 / California zone 4 (ftUS)
    (66, 22, 2229),   // NAD83 / California zone 5 (ftUS)
    (66, 23, 2230),   // NAD83 / California zone 6 (ftUS)
    (66, 24, 2231),   // NAD83 / Colorado North (ftUS)
    (66, 25, 2232),   // NAD83 / Colorado Central (ftUS)
    (66, 26, 2233),   // NAD83 / Colorado South (ftUS)
    (66, 27, 2234),   // NAD83 / Connecticut (ftUS)
    (66, 28, 2235),   // NAD83 / Delaware (ftUS)
    (66, 29, 2236),   // NAD83 / Florida East (ftUS)
    (66, 30, 2237),   // NAD83 / Florida West (ftUS)
    (66, 31, 2238),   // NAD83 / Florida North (ftUS)
    (66, 32, 2239),   // NAD83 / Georgia East (ftUS)
    (66, 33, 2240),   // NAD83 / Georgia West (ftUS)
    (66, 36, 3759),   // NAD83 / Hawaii zone 3 (ftUS)
    (66, 39, 2241),   // NAD83 / Idaho East (ftUS)
    (66, 40, 2242),   // NAD83 / Idaho Central (ftUS)
    (66, 41, 2243),   // NAD83 / Idaho West (ftUS)
    (66, 42, 3435),   // NAD83 / Illinois East (ftUS)
    (66, 43, 3436),   // NAD83 / Illinois West (ftUS)
    (66, 44, 2244),   // NAD83 / Indiana East (ftUS) (deprecated record)
    (66, 45, 2245),   // NAD83 / Indiana West (ftUS) (deprecated record)
    (66, 46, 3417),   // NAD83 / Iowa North (ftUS)
    (66, 47, 3418),   // NAD83 / Iowa South (ftUS)
    (66, 48, 3419),   // NAD83 / Kansas North (ftUS)
    (66, 49, 3420),   // NAD83 / Kansas South (ftUS)
    (66, 50, 2246),   // NAD83 / Kentucky North (ftUS)
    (66, 51, 2247),   // NAD83 / Kentucky South (ftUS)
    (66, 52, 3451),   // NAD83 / Louisiana North (ftUS)
    (66, 53, 3452),   // NAD83 / Louisiana South (ftUS)
    (66, 54, 3453),   // NAD83 / Louisiana Offshore (ftUS)
    (66, 55, 26847),  // NAD83 / Maine East (ftUS)
    (66, 56, 26848),  // NAD83 / Maine West (ftUS)
    (66, 57, 2248),   // NAD83 / Maryland (ftUS)
    (66, 58, 2249),   // NAD83 / Massachusetts Mainland (ftUS)
    (66, 59, 2250),   // NAD83 / Massachusetts Island (ftUS)
    (66, 60, 2251),   // NAD83 / Michigan North (ft)
    (66, 61, 2252),   // NAD83 / Michigan Central (ft)
    (66, 62, 2253),   // NAD83 / Michigan South (ft)
    (66, 63, 26849),  // NAD83 / Minnesota North (ftUS)
    (66, 64, 26850),  // NAD83 / Minnesota Central (ftUS)
    (66, 65, 26851),  // NAD83 / Minnesota South (ftUS)
    (66, 66, 2254),   // NAD83 / Mississippi East (ftUS)
    (66, 67, 2255),   // NAD83 / Mississippi West (ftUS)
    (66, 71, 2256),   // NAD83 / Montana (ft)
    (66, 72, 26852),  // NAD83 / Nebraska (ftUS)
    (66, 73, 3421),   // NAD83 / Nevada East (ftUS)
    (66, 74, 3422),   // NAD83 / Nevada Central (ftUS)
    (66, 75, 3423),   // NAD83 / Nevada West (ftUS)
    (66, 76, 3437),   // NAD83 / New Hampshire (ftUS)
    (66, 77, 3424),   // NAD83 / New Jersey (ftUS)
    (66, 78, 2257),   // NAD83 / New Mexico East (ftUS)
    (66, 79, 2258),   // NAD83 / New Mexico Central (ftUS)
    (66, 80, 2259),   // NAD83 / New Mexico West (ftUS)
    (66, 81, 2260),   // NAD83 / New York East (ftUS)
    (66, 82, 2261),   // NAD83 / New York Central (ftUS)
    (66, 83, 2262),   // NAD83 / New York West (ftUS)
    (66, 84, 2263),   // NAD83 / New York Long Island (ftUS)
    (66, 85, 2264),   // NAD83 / North Carolina (ftUS)
    (66, 86, 2265),   // NAD83 / North Dakota North (ft)
    (66, 87, 2266),   // NAD83 / North Dakota South (ft)
    (66, 88, 3734),   // NAD83 / Ohio North (ftUS)
    (66, 89, 3735),   // NAD83 / Ohio South (ftUS)
    (66, 90, 2267),   // NAD83 / Oklahoma North (ftUS)
    (66, 91, 2268),   // NAD83 / Oklahoma South (ftUS)
    (66, 92, 2269),   // NAD83 / Oregon North (ft)
    (66, 93, 2270),   // NAD83 / Oregon South (ft)
    (66, 94, 2271),   // NAD83 / Pennsylvania North (ftUS)
    (66, 95, 2272),   // NAD83 / Pennsylvania South (ftUS)
    (66, 96, 3438),   // NAD83 / Rhode Island (ftUS)
    (66, 97, 2273),   // NAD83 / South Carolina (ft)
    (66, 98, 3454),   // NAD83 / South Dakota North (ftUS) (deprecated record)
    (66, 99, 3455),   // NAD83 / South Dakota South (ftUS)
    (66, 100, 2274),  // NAD83 / Tennessee (ftUS)
    (66, 101, 2275),  // NAD83 / Texas North (ftUS)
    (66, 102, 2276),  // NAD83 / Texas North Central (ftUS)
    (66, 103, 2277),  // NAD83 / Texas Central (ftUS)
    (66, 104, 2278),  // NAD83 / Texas South Central (ftUS)
    (66, 105, 2279),  // NAD83 / Texas South (ftUS)
    (66, 106, 3560),  // NAD83 / Utah North (ftUS)
    (66, 107, 3566),  // NAD83 / Utah Central (ftUS)
    (66, 108, 3567),  // NAD83 / Utah South (ftUS)
    (66, 110, 2283),  // NAD83 / Virginia North (ftUS)
    (66, 111, 2284),  // NAD83 / Virginia South (ftUS)
    (66, 112, 2285),  // NAD83 / Washington North (ftUS)
    (66, 113, 2286),  // NAD83 / Washington South (ftUS)
    (66, 114, 26853), // NAD83 / West Virginia North (ftUS)
    (66, 115, 26854), // NAD83 / West Virginia South (ftUS)
    (66, 116, 2287),  // NAD83 / Wisconsin North (ftUS)
    (66, 117, 2288),  // NAD83 / Wisconsin Central (ftUS)
    (66, 118, 2289),  // NAD83 / Wisconsin South (ftUS)
    (66, 119, 3736),  // NAD83 / Wyoming East (ftUS)
    (66, 120, 3737),  // NAD83 / Wyoming East Central (ftUS)
    (66, 121, 3738),  // NAD83 / Wyoming West Central (ftUS)
    (66, 122, 3739),  // NAD83 / Wyoming West (ftUS)
    (63, 1, 25828),   // ETRS89 / UTM zone 28N
    (63, 2, 25829),   // ETRS89 / UTM zone 29N
    (63, 3, 25830),   // ETRS89 / UTM zone 30N
    (63, 4, 25831),   // ETRS89 / UTM zone 31N
    (63, 5, 25832),   // ETRS89 / UTM zone 32N
    (63, 6, 25833),   // ETRS89 / UTM zone 33N
    (63, 7, 25834),   // ETRS89 / UTM zone 34N
    (63, 8, 25835),   // ETRS89 / UTM zone 35N
    (63, 9, 25836),   // ETRS89 / UTM zone 36N
    (63, 10, 25837),  // ETRS89 / UTM zone 37N
    (63, 11, 25838),  // ETRS_1989_UTM_Zone_38N
    (62, 1, 26901),   // NAD83 / UTM zone 1N
    (62, 2, 26902),   // NAD83 / UTM zone 2N
    (62, 4, 26903),   // NAD83 / UTM zone 3N
    (62, 5, 26904),   // NAD83 / UTM zone 4N
    (62, 6, 26905),   // NAD83 / UTM zone 5N
    (62, 7, 26906),   // NAD83 / UTM zone 6N
    (62, 8, 26907),   // NAD83 / UTM zone 7N
    (62, 9, 26908),   // NAD83 / UTM zone 8N
    (62, 10, 26909),  // NAD83 / UTM zone 9N
    (62, 11, 26910),  // NAD83 / UTM zone 10N
    (62, 12, 26911),  // NAD83 / UTM zone 11N
    (62, 13, 26912),  // NAD83 / UTM zone 12N
    (62, 14, 26913),  // NAD83 / UTM zone 13N
    (62, 15, 26914),  // NAD83 / UTM zone 14N
    (62, 16, 26915),  // NAD83 / UTM zone 15N
    (62, 17, 26916),  // NAD83 / UTM zone 16N
    (62, 18, 26917),  // NAD83 / UTM zone 17N
    (62, 19, 26918),  // NAD83 / UTM zone 18N
    (62, 20, 26919),  // NAD83 / UTM zone 19N
    (62, 21, 26920),  // NAD83 / UTM zone 20N
    (62, 22, 26921),  // NAD83 / UTM zone 21N
    (62, 23, 26922),  // NAD83 / UTM zone 22N
    (62, 24, 26923),  // NAD83 / UTM zone 23N
    (65, 0, 3857),    // WGS 84 / Pseudo-Mercator
];
