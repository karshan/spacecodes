#version 330

#define MAX_LIGHTS              4
#define LIGHT_DIRECTIONAL       0
#define LIGHT_POINT             1
#define PI 3.14159265358979323846

struct Light {
    int enabled;
    int type;
    vec3 position;
    vec3 target;
    vec4 color;
    float intensity;
};

// Input vertex attributes (from vertex shader)
in vec3 fragPosition;
in vec2 fragTexCoord;
in vec4 fragColor;
in vec3 fragNormal;
in vec4 shadowPos;
in mat3 TBN;

// Output fragment color
out vec4 finalColor;

// Input uniform values
uniform int numOfLights;
uniform sampler2D albedoMap;
uniform sampler2D normalMap;
uniform sampler2D emissiveMap; // g:emissive

uniform vec2 tiling;
uniform vec2 offset;

uniform int useTexAlbedo;
uniform int useTexNormal;
uniform int useTexEmissive;

uniform vec4  albedoColor;
uniform vec4  emissiveColor;
uniform float emissivePower;

// Input lighting values
uniform Light lights[MAX_LIGHTS];
uniform vec3 viewPos;

uniform int useAo;
uniform vec3 cubePos[20];
uniform vec3 cubeSize;
uniform int numCubes;

uniform vec3 gcubePos[2];
uniform float gcubeSize[2];

uniform int useHdrToneMap;
uniform int useGamma;
uniform float lightMult;
uniform float ao_intensity;
uniform float ao_stepsize;
uniform int ao_iterations;

uniform float shadow_mint;
uniform float shadow_maxt;
uniform float shadow_w;
uniform float shadow_intensity;
uniform vec3 shadow_light;

float sdBox(vec3 p, vec3 b) {
  vec3 q = abs(p) - b;
  return length(max(q,0.0)) + min(max(q.x,max(q.y,q.z)),0.0);
}

float map(in vec3 pos) {
    float result = 1000.0;
    for (int i = 0; i < numCubes; i++) {
        result = min(result, sdBox(pos - cubePos[i], cubeSize/2));
    }
    result = min(result, sdBox(pos - gcubePos[0], cubeSize*gcubeSize[0]/2));
    result = min(result, sdBox(pos - gcubePos[1], cubeSize*gcubeSize[1]/2));
    return result;
}

// https://iquilezles.org/articles/nvscene2008/rwwtt.pdf
float calcAO(in vec3 pos, in vec3 nor)
{
	float occ = 0.0;
    for( int i=1; i<=ao_iterations; i++ )
    {
        float h = ao_stepsize*float(i);
        float d = map(pos + h*nor);
        occ += max(0.0, (h-d)/h);
        // occ += (h-d)*sca;
        // sca *= 0.95;
        // if( occ>0.35 ) break;
    }
    return (1.0 - occ * ao_intensity);
    // return clamp( 1.0 - 3.0*occ, 0.0, 1.0 ) * (0.5+0.5*nor.z);
}

float softshadow( in vec3 ro, in vec3 rd, float mint, float maxt, float k )
{
    float res = 1.0;
    float t = mint;
    for( int i=0; i<256 && t<maxt; i++ )
    {
        float h = map(ro + rd*t);
        if( h<0.001 )
            return 0.0;
        res = min( res, k*h/t );
        t += h;
    }
    return res;
}


vec4 ComputePBR()
{
    vec3 albedo = albedoColor.rgb;
    float albedoAlpha = albedoColor.a;
    if (useTexAlbedo == 1) {
        vec4 albedoRGBA = texture(albedoMap,vec2(fragTexCoord.x*tiling.x + offset.x, fragTexCoord.y*tiling.y + offset.y)).rgba;
        albedo = albedoRGBA.rgb;
        albedoAlpha = albedoRGBA.a;
        albedo = vec3(albedoColor.x*albedo.x, albedoColor.y*albedo.y, albedoColor.z*albedo.z);
    }

    vec3 N = normalize(fragNormal);
    if (useTexNormal == 1)
    {
        N = texture(normalMap, vec2(fragTexCoord.x*tiling.x + offset.y, fragTexCoord.y*tiling.y + offset.y)).rgb;
        N = normalize(N*2.0 - 1.0);
        N = normalize(N*TBN);
    }

    // vec3 emissive = vec3(0);
    // emissive = (texture(emissiveMap, vec2(fragTexCoord.x*tiling.x+offset.x, fragTexCoord.y*tiling.y+offset.y)).rgb).g * emissiveColor.rgb*emissivePower * useTexEmissive;

    vec3 lightAccum = vec3(0.0);
    for (int i = 0; i < numOfLights; i++)
    {
        vec3 L = normalize(lights[i].position - fragPosition);
        if (lights[i].type == LIGHT_DIRECTIONAL)
        {
            L = -normalize(lights[i].target - lights[i].position);
        }

        float dist = length(lights[i].position - fragPosition);
        float attenuation = 1.0/(dist*dist*0.23);
        
        if (lights[i].type == LIGHT_DIRECTIONAL) {
            attenuation = 1.0;
        }

        float occ = calcAO(fragPosition, N);

        float diffuse = clamp(dot(N, L), 0.0, 1.0);
        if (useAo == 1)
            diffuse *= occ;
        float shadow = 1.0;
        if (lights[i].type == LIGHT_DIRECTIONAL) {
            shadow = softshadow(fragPosition, normalize(shadow_light), shadow_mint, shadow_maxt, shadow_w) * 0.5 + 0.5;
            shadow = pow(shadow, shadow_intensity);
            diffuse *= shadow;
        }
        lightAccum += (albedo * lightMult * diffuse * lights[i].color.rgb) * lights[i].enabled; // light color constant
    }
    // float t = length(viewPos - fragPosition);
    // lightAccum = mix( lightAccum, vec3(0.7,0.7,0.9), 1.0-exp( -0.0001*t*t*t ) );
    if (lights[0].enabled == 0) {
        return vec4(albedo, albedoAlpha);
    } else {
        return vec4(lightAccum + emissiveColor.rgb * emissivePower, albedoAlpha);
    }
}

void main()
{
    vec4 color4 = ComputePBR();
    vec3 color = color4.rgb;

    // HDR tonemapping
    if (useHdrToneMap == 1) {
        color = pow(color, color + vec3(1.0));
    }
    
    // Gamma correction
    if (useGamma == 1) {
        color = pow(color, vec3(1.0/2.2));
    }

    finalColor = vec4(color, color4.a);
}