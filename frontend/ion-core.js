/* ══════════════════════════════════════════════════════════════════
   ION CORE — the organic energy orb, ported from the ION portfolio
   hero (three.js 0.160.0, vendored locally). Palette re-tuned to the
   Ion Sense green identity and wired to the settings window's
   core-state contract:

     body[data-core-state]  "idle" | "focus" | "alert"
       focus = detectors restarting, alert = test event fired.
       Energy eases in JS (uEnergy) — the orb brightens and its
       surface breathes harder; no palette change.

   Layers (verbatim from the site original):
     1  CORE    — icosahedron displaced by 3 octaves of 3D simplex
                   noise, colored by displacement + fresnel rim
     2  SHELL   — back-side glass sphere, additive fresnel halo
     3  SPARKS  — 600-point orbital particle field (300 on mobile)
     4  GLOW    — background radial breathing glow (edge-masked)

   Safety: prefers-reduced-motion → static fallback disc; WebGL
   failure → fallback disc; pauses when the popover window is
   hidden or the wrap is scrolled out of view.
   ══════════════════════════════════════════════════════════════════ */

import * as THREE from "three";

const REDUCED = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const IS_MOBILE = window.matchMedia("(max-width: 768px)").matches;

/* Ion Sense palette — one green family, energy only, never hue. */
const PALETTE = {
  core: "#1B7A38",
  mid: "#34C759",
  edge: "#5CE87C",
  hot: "#DFF5E4",
  shell: "#34C759",
  glow: "#34C759",
};

/* 3D simplex noise (Ashima / Ian McEwan) — injected into the core's
   vertex shader; this is what sculpts the living blob surface. */
const NOISE_GLSL = `
  vec3 mod289(vec3 x){return x-floor(x*(1.0/289.0))*289.0;}
  vec4 mod289(vec4 x){return x-floor(x*(1.0/289.0))*289.0;}
  vec4 permute(vec4 x){return mod289(((x*34.0)+1.0)*x);}
  vec4 taylorInvSqrt(vec4 r){return 1.79284291400159-0.85373472095314*r;}
  float snoise(vec3 v){
    const vec2 C=vec2(1.0/6.0,1.0/3.0);
    const vec4 D=vec4(0.0,0.5,1.0,2.0);
    vec3 i=floor(v+dot(v,C.yyy));
    vec3 x0=v-i+dot(i,C.xxx);
    vec3 g=step(x0.yzx,x0.xyz);
    vec3 l=1.0-g;
    vec3 i1=min(g.xyz,l.zxy);
    vec3 i2=max(g.xyz,l.zxy);
    vec3 x1=x0-i1+C.xxx;
    vec3 x2=x0-i2+C.yyy;
    vec3 x3=x0-D.yyy;
    i=mod289(i);
    vec4 p=permute(permute(permute(
      i.z+vec4(0.0,i1.z,i2.z,1.0))
      +i.y+vec4(0.0,i1.y,i2.y,1.0))
      +i.x+vec4(0.0,i1.x,i2.x,1.0));
    float n_=0.142857142857;
    vec3 ns=n_*D.wyz-D.xzx;
    vec4 j=p-49.0*floor(p*ns.z*ns.z);
    vec4 x_=floor(j*ns.z);
    vec4 y_=floor(j-7.0*x_);
    vec4 x=x_*ns.x+ns.yyyy;
    vec4 y=y_*ns.x+ns.yyyy;
    vec4 h=1.0-abs(x)-abs(y);
    vec4 b0=vec4(x.xy,y.xy);
    vec4 b1=vec4(x.zw,y.zw);
    vec4 s0=floor(b0)*2.0+1.0;
    vec4 s1=floor(b1)*2.0+1.0;
    vec4 sh=-step(h,vec4(0.0));
    vec4 a0=b0.xzyw+s0.xzyw*sh.xxyy;
    vec4 a1=b1.xzyw+s1.xzyw*sh.zzww;
    vec3 p0=vec3(a0.xy,h.x);
    vec3 p1=vec3(a0.zw,h.y);
    vec3 p2=vec3(a1.xy,h.z);
    vec3 p3=vec3(a1.zw,h.w);
    vec4 norm=taylorInvSqrt(vec4(dot(p0,p0),dot(p1,p1),dot(p2,p2),dot(p3,p3)));
    p0*=norm.x;p1*=norm.y;p2*=norm.z;p3*=norm.w;
    vec4 m=max(0.6-vec4(dot(x0,x0),dot(x1,x1),dot(x2,x2),dot(x3,x3)),0.0);
    m=m*m;
    return 42.0*dot(m*m,vec4(dot(p0,x0),dot(p1,x1),dot(p2,x2),dot(p3,x3)));
  }
`;

const CoreEnergy = {
  init() {
    const canvas = document.getElementById("ion-core-canvas");
    const wrap = document.querySelector(".hero-core-wrap");
    const fallback = document.querySelector(".ion-core-fallback");
    if (!canvas || !fallback || REDUCED) {
      if (canvas) canvas.style.display = "none";
      if (fallback) fallback.style.opacity = "0.6";
      return;
    }

    try {
      const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true, powerPreference: "high-performance" });
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      renderer.setSize(canvas.clientWidth || 148, canvas.clientHeight || 148);

      /* WebGL is live — retire the static fallback disc so it does not
         read as a hard-edged circle behind the shader. */
      fallback.style.transition = "opacity 1.2s cubic-bezier(0.16, 1, 0.3, 1)";
      fallback.style.opacity = "0";

      const scene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(45, (canvas.clientWidth || 148) / (canvas.clientHeight || 148), 0.1, 100);
      camera.position.z = 4;

      const clock = new THREE.Clock();
      let mouseX = 0, mouseY = 0, targetMouseX = 0, targetMouseY = 0;
      let isIntersecting = true;
      let energy = 0, targetEnergy = 0;

      const readCoreState = () => {
        const state = document.body.dataset.coreState;
        targetEnergy = state === "alert" ? 1 : state === "focus" ? 0.6 : 0;
      };
      readCoreState();
      const stateObserver = new MutationObserver(readCoreState);
      stateObserver.observe(document.body, { attributes: true, attributeFilter: ["data-core-state"] });

      /* Core Energy Sphere */
      const coreGeometry = new THREE.IcosahedronGeometry(1, IS_MOBILE ? 32 : 64);
      const coreMaterial = new THREE.ShaderMaterial({
        uniforms: {
          uTime: { value: 0 },
          uMouse: { value: new THREE.Vector2(0, 0) },
          uEnergy: { value: 0 },
          uColorCore: { value: new THREE.Color(PALETTE.core) },
          uColorMid: { value: new THREE.Color(PALETTE.mid) },
          uColorEdge: { value: new THREE.Color(PALETTE.edge) },
          uColorHot: { value: new THREE.Color(PALETTE.hot) },
        },
        vertexShader: `
          uniform float uTime;
          uniform vec2 uMouse;
          uniform float uEnergy;
          varying vec3 vNormal;
          varying vec3 vPosition;
          varying float vDisplacement;
          varying float vFresnel;
          ${NOISE_GLSL}

          void main() {
            vec3 pos = position;
            float t = uTime * 0.3;
            float n1 = snoise(pos * 1.2 + vec3(t * 0.5)) * 0.18;
            float n2 = snoise(pos * 2.8 + vec3(t * 0.8, t * 0.3, t * 0.6)) * 0.09;
            float n3 = snoise(pos * 5.5 - vec3(t * 0.3)) * 0.04;
            float displacement = (n1 + n2 + n3) * (1.0 + uEnergy * 0.9);
            vDisplacement = displacement;
            pos += normal * displacement;

            float mouseDist = length(uMouse);
            pos += normal * mouseDist * 0.05;

            vec4 worldPos = modelMatrix * vec4(pos, 1.0);
            vNormal = normalize(normalMatrix * normal);
            vPosition = worldPos.xyz;
            vec3 viewDir = normalize(cameraPosition - worldPos.xyz);
            vFresnel = 1.0 - max(dot(viewDir, vNormal), 0.0);

            gl_Position = projectionMatrix * viewMatrix * worldPos;
          }
        `,
        fragmentShader: `
          uniform float uTime;
          uniform float uEnergy;
          uniform vec3 uColorCore;
          uniform vec3 uColorMid;
          uniform vec3 uColorEdge;
          uniform vec3 uColorHot;
          varying vec3 vNormal;
          varying vec3 vPosition;
          varying float vDisplacement;
          varying float vFresnel;

          void main() {
            float fresnel = pow(vFresnel, 2.0);
            float d = vDisplacement * 2.5 + 0.5;
            vec3 color = mix(uColorCore, uColorMid, smoothstep(0.0, 0.6, d));
            color = mix(color, uColorEdge, fresnel * 0.8);
            float hot = smoothstep(0.15, 0.25, vDisplacement);
            color = mix(color, uColorHot, hot * 0.4);
            float inner = 1.0 - fresnel;
            color += uColorCore * inner * 0.15;
            float pulse = sin(uTime * 0.5) * 0.08 + 0.92;
            color *= pulse * (1.0 + uEnergy * 0.35);
            float alpha = fresnel * 0.6 + 0.4;
            gl_FragColor = vec4(color, alpha);
          }
        `,
        transparent: true,
        depthWrite: false,
        blending: THREE.NormalBlending,
      });

      const coreMesh = new THREE.Mesh(coreGeometry, coreMaterial);
      scene.add(coreMesh);

      /* Outer Glass Shell */
      const shellGeometry = new THREE.IcosahedronGeometry(1.35, IS_MOBILE ? 16 : 32);
      const shellMaterial = new THREE.ShaderMaterial({
        uniforms: { uTime: { value: 0 }, uColor: { value: new THREE.Color(PALETTE.shell) } },
        vertexShader: `
          varying float vFresnel;
          void main() {
            vec4 worldPos = modelMatrix * vec4(position, 1.0);
            vNormal = normalize(normalMatrix * normal);
            vec3 viewDir = normalize(cameraPosition - worldPos.xyz);
            vFresnel = 1.0 - max(dot(viewDir, vNormal), 0.0);
            gl_Position = projectionMatrix * viewMatrix * worldPos;
          }
        `,
        fragmentShader: `
          uniform float uTime;
          uniform vec3 uColor;
          varying float vFresnel;
          void main() {
            float fresnel = pow(vFresnel, 3.0);
            float pulse = sin(uTime * 0.4) * 0.1 + 0.9;
            vec3 color = uColor * fresnel * pulse;
            float alpha = fresnel * 0.3;
            gl_FragColor = vec4(color, alpha);
          }
        `,
        transparent: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
        side: THREE.BackSide,
      });
      const shellMesh = new THREE.Mesh(shellGeometry, shellMaterial);
      scene.add(shellMesh);

      /* Particle Field */
      const particleCount = IS_MOBILE ? 300 : 600;
      const positions = new Float32Array(particleCount * 3);
      const colors = new Float32Array(particleCount * 3);
      const sizes = new Float32Array(particleCount);
      const speeds = new Float32Array(particleCount);
      const colorPalette = [
        new THREE.Color(PALETTE.mid), new THREE.Color(PALETTE.edge),
        new THREE.Color("#3DD968"), new THREE.Color(PALETTE.hot),
      ];

      for (let i = 0; i < particleCount; i++) {
        const radius = 1.6 + Math.random() * 1.8;
        const theta = Math.random() * Math.PI * 2;
        const phi = Math.acos(2 * Math.random() - 1);
        positions[i * 3] = radius * Math.sin(phi) * Math.cos(theta);
        positions[i * 3 + 1] = radius * Math.sin(phi) * Math.sin(theta);
        positions[i * 3 + 2] = radius * Math.cos(phi);
        const c = colorPalette[Math.floor(Math.random() * colorPalette.length)];
        colors[i * 3] = c.r; colors[i * 3 + 1] = c.g; colors[i * 3 + 2] = c.b;
        sizes[i] = Math.random() * 0.03 + 0.01;
        speeds[i] = Math.random() * 0.5 + 0.5;
      }

      const particleGeometry = new THREE.BufferGeometry();
      particleGeometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
      particleGeometry.setAttribute("aColor", new THREE.BufferAttribute(colors, 3));
      particleGeometry.setAttribute("aSize", new THREE.BufferAttribute(sizes, 1));
      particleGeometry.setAttribute("aSpeed", new THREE.BufferAttribute(speeds, 1));

      const particleMaterial = new THREE.ShaderMaterial({
        uniforms: { uTime: { value: 0 }, uMouse: { value: new THREE.Vector2(0, 0) } },
        vertexShader: `
          attribute vec3 aColor;
          attribute float aSize;
          attribute float aSpeed;
          uniform float uTime;
          uniform vec2 uMouse;
          varying vec3 vColor;
          varying float vAlpha;
          void main() {
            vec3 pos = position;
            float angle = uTime * 0.1 * aSpeed;
            float c = cos(angle);
            float s = sin(angle);
            pos.x = position.x * c - position.z * s;
            pos.z = position.x * s + position.z * c;
            pos.y += sin(uTime * 0.5 + position.x * 3.0) * 0.05;
            float mouseDist = length(uMouse);
            pos.xy += uMouse * 0.1 * (1.0 / length(position)) * mouseDist;
            vec4 mvPosition = modelViewMatrix * vec4(pos, 1.0);
            gl_PointSize = aSize * 300.0 / -mvPosition.z;
            gl_Position = projectionMatrix * mvPosition;
            vColor = aColor;
            float dist = length(pos);
            /* Well-defined smoothstep orientation (edge0 < edge1) — reversed
               ranges are undefined behavior in GLSL and break on some drivers. */
            vAlpha = (1.0 - smoothstep(1.8, 3.5, dist)) * smoothstep(1.5, 2.0, dist);
          }
        `,
        fragmentShader: `
          varying vec3 vColor;
          varying float vAlpha;
          void main() {
            vec2 center = gl_PointCoord - 0.5;
            float dist = length(center);
            if (dist > 0.5) discard;
            float alpha = (1.0 - dist * 2.0);
            alpha = pow(alpha, 2.0) * vAlpha;
            gl_FragColor = vec4(vColor, alpha * 0.8);
          }
        `,
        transparent: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
      });

      const particles = new THREE.Points(particleGeometry, particleMaterial);
      scene.add(particles);

      /* Ambient Glow */
      const glowGeometry = new THREE.PlaneGeometry(6, 6);
      const glowMaterial = new THREE.ShaderMaterial({
        uniforms: { uTime: { value: 0 }, uColor: { value: new THREE.Color(PALETTE.glow) } },
        vertexShader: `varying vec2 vUv; void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }`,
        fragmentShader: `uniform float uTime; uniform vec3 uColor; varying vec2 vUv;
          void main() {
            vec2 center = vUv - 0.5;
            float dist = length(center);
            /* Edge mask — the glow must reach exactly 0 at the CANVAS border.
               The plane (±3) is larger than the frustum (±2.07 at this depth),
               so the canvas edge sits at uv-radius ≈ 0.345 — the mask must
               finish fading BEFORE that radius, or the additive alpha
               accumulates across the whole canvas and composites as a
               visible rectangle on the page. */
            float m = length(vUv - 0.5);
            float mask = 1.0 - smoothstep(0.16, 0.34, m);
            float glow = exp(-dist * 4.0) * 0.15 * mask;
            float pulse = sin(uTime * 0.5) * 0.03 + 0.97;
            glow *= pulse;
            gl_FragColor = vec4(uColor, glow);
          }`,
        transparent: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
      });
      const glowMesh = new THREE.Mesh(glowGeometry, glowMaterial);
      glowMesh.position.z = -1;
      scene.add(glowMesh);

      /* Debug hook (harmless in production; renderer/camera allow driving a
         frame manually from tests when rAF is throttled). */
      window.__ionCore = { scene, core: coreMesh, shell: shellMesh, particles, glow: glowMesh, renderer, camera };

      window.addEventListener("mousemove", (e) => {
        targetMouseX = (e.clientX / window.innerWidth - 0.5) * 2;
        targetMouseY = -(e.clientY / window.innerHeight - 0.5) * 2;
      });

      const onResize = () => {
        const w = Math.max(1, canvas.clientWidth);
        const h = Math.max(1, canvas.clientHeight);
        renderer.setSize(w, h, false);
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
      };
      window.addEventListener("resize", onResize);
      onResize();

      const observer = new IntersectionObserver((entries) => {
        isIntersecting = entries[0].isIntersecting;
      }, { threshold: 0.01 });
      if (wrap) observer.observe(wrap);

      const windowVisible = () => {
        const state = document.body.dataset.windowState;
        return state === "active" || state === "entering";
      };

      const animate = () => {
        requestAnimationFrame(animate);
        if (!isIntersecting || !windowVisible() || !renderer) return;

        const time = clock.getElapsedTime();
        mouseX += (targetMouseX - mouseX) * 0.05;
        mouseY += (targetMouseY - mouseY) * 0.05;
        energy += (targetEnergy - energy) * 0.06;

        coreMesh.material.uniforms.uTime.value = time;
        coreMesh.material.uniforms.uMouse.value.set(mouseX, mouseY);
        coreMesh.material.uniforms.uEnergy.value = energy;
        coreMesh.rotation.y = time * 0.08 + mouseX * 0.3;
        coreMesh.rotation.x = mouseY * 0.2;

        shellMesh.material.uniforms.uTime.value = time;
        shellMesh.rotation.y = -time * 0.05;
        shellMesh.rotation.x = mouseY * 0.1;

        particles.material.uniforms.uTime.value = time;
        particles.material.uniforms.uMouse.value.set(mouseX, mouseY);
        particles.rotation.y = time * 0.02;

        glowMesh.material.uniforms.uTime.value = time;

        renderer.render(scene, camera);
      };
      animate();
    } catch (error) {
      console.warn("Ion Sense core WebGL initialization failed:", error);
      canvas.style.display = "none";
      fallback.style.opacity = "0.6";
    }
  }
};

CoreEnergy.init();
