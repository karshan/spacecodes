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

float sdBox(vec3 p, vec3 b) {
  vec3 q = abs(p) - b;
  return length(max(q,0.0)) + min(max(q.x,max(q.y,q.z)),0.0);
}

float map(in vec3 pos) {
    float result = 1000.0;
    for (int i = 0; i < numCubes; i++) {
        result = min(result, sdBox(pos - cubePos[i], cubeSize/2.5));
    }
    return result;
}

// https://iquilezles.org/articles/nvscene2008/rwwtt.pdf
float calcAO( in vec3 pos, in vec3 nor )
{
	float occ = 0.0;
    float sca = 0.15;
    for( int i=0; i<15; i++ )
    {
        float h = 0.01 + 0.12*float(i)/4.0;
        float d = map( pos + h*nor );
        occ += (h-d)*sca;
        sca *= 0.95;
        if( occ>0.35 ) break;
    }
    return clamp( 1.0 - 3.0*occ, 0.0, 1.0 ) * (0.5+0.5*nor.z);
}

vec3 ComputePBR()
{
    vec3 albedo = albedoColor.rgb;
    if (useTexAlbedo == 1) {
        albedo = texture(albedoMap,vec2(fragTexCoord.x*tiling.x + offset.x, fragTexCoord.y*tiling.y + offset.y)).rgb;
        albedo = vec3(albedoColor.x*albedo.x, albedoColor.y*albedo.y, albedoColor.z*albedo.z);
    }

    vec3 N = normalize(fragNormal);
    if (useTexNormal == 1)
    {
        N = texture(normalMap, vec2(fragTexCoord.x*tiling.x + offset.y, fragTexCoord.y*tiling.y + offset.y)).rgb;
        N = normalize(N*2.0 - 1.0);
        N = normalize(N*TBN);
    }

    vec3 emissive = vec3(0);
    emissive = (texture(emissiveMap, vec2(fragTexCoord.x*tiling.x+offset.x, fragTexCoord.y*tiling.y+offset.y)).rgb).g * emissiveColor.rgb*emissivePower * useTexEmissive;

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
        lightAccum += (albedo * 2.20 * diffuse * vec3(1.30,1.00,0.70)) * lights[i].enabled; // light color constant
    }
    float t = length(viewPos - fragPosition);
    lightAccum = mix( lightAccum, vec3(0.7,0.7,0.9), 1.0-exp( -0.0001*t*t*t ) );
    return lightAccum + emissive;
}

void main()
{
    vec3 color = ComputePBR();

    // HDR tonemapping
    color = pow(color, color + vec3(1.0));
    
    // Gamma correction
    color = pow(color, vec3(1.0/2.2));

    finalColor = vec4(color, 1.0);
}