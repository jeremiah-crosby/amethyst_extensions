#version 150 core

uniform sampler2D TilesheetTexture;
uniform sampler2D TilemapTexture;

const int TILEMAP_BUF_LENGTH = 4096;

layout (std140) uniform TileMapBuffer {
    vec4 u_Data[TILEMAP_BUF_LENGTH];
};

layout (std140) uniform FragmentArgs {
    vec4 u_WorldSize;
    vec4 u_TilesheetSize;
    uint tilesetCount;
};

in VertexData {
    vec4 position;
    vec3 normal;
    vec3 tangent;
    vec2 tex_coord;
} vertex;

out vec4 Color;

void main() {

    // Step 1) I use GL so I have to flip my UV
    vec2 flipped_uv = vec2( vertex.tex_coord.x, 1.0 - vertex.tex_coord.y );

    // Step 2) Retrieve the index into the tileset
    // Remember I mentioned that I used Unity which requires Float4's for all textures
    // If you can have a single float for each "pixel" in your texture there is no need
    // for the .x at the end.
    int index = int(texture(TilemapTexture, flipped_uv) * tilesetCount);

    if (index == 0) {
        discard;
    }

    // Step 3) Get the X and Y coordinates in the tileset
    int xpos = index % int(u_TilesheetSize.x);
    int ypos = index / int(u_TilesheetSize.y);

    // Step 3a) We increment the y position by one to account for the fact
    // that GL reads UV's from the bottom left.
    // A Y coordinate of 0.0 is the top of the image.
    // We want to read from the bottom left so we fix our UV to have the Y on the bottom.
    ypos += 1;

    // Step 4) Find the starting UV coordinate.
    // We divide by the size in tiles to take a coordinate like:
    // X = 4, Y = 2 on a 8 by 3 tileset into, (1/2, 2/3)
    // Normalizing the X and Y coordinate into the UV space.
    vec2 uv = vec2(xpos, ypos) / u_TilesheetSize.xy;

    // Step 5) Determine the offset into the tile for this fragment.
    // What we use here is the "fraction" operator. It returns only the fractional
    // part of a decimal value.
    // We get the actual UV coordinate and then multiply it by the size in tiles.
    // This gives us a non-normalized tilemap location ie it is within the bounds
    // of [0, map_size) on the x and y axis.
    // The fractional portion is then the amount between 0 and 1 within the tile.
    // Then the last division by the tileset size is bringing us into the "tile-space"
    // or the actual size of a tile between a normalized coordinate system on the tileset image.
    float xoffset = fract( vertex.tex_coord.x * u_WorldSize.x ) / u_TilesheetSize.x;
    float yoffset = fract( vertex.tex_coord.y * u_WorldSize.y ) / u_TilesheetSize.y;

    // We can add this to the UV now. However remember that GL is bottom-left based so we
    // will subtract Y from the uv position.
    uv += vec2( xoffset, -yoffset );

    // Step 6) Match any missing image size.
    //uv *= image_ratio;
    
    // Step 7) Return to the normal UV coordinate space if we are in GL.
    uv.y = 1.0f - uv.y;
    
    // Step 8) Return the final color.
    Color = texture(TilesheetTexture, uv);
}