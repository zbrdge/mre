import io::{reader, reader_util};
import result::{ok, err, extensions};

import std::map::{hashmap, str_hash, hash_from_strs};
import std::json;

import elasticsearch::{client, search_builder, index_builder, json_dict_builder};
import mongrel2::{connection, request};
import mre::mre;
import mre::response::{response, http_200, http_400, http_404, redirect};
import mu_context = mustache::context;
import mustache::to_mustache;
import zmq_context = zmq::context;
import zmq::error;

import post::{post};

// FIXME: move after https://github.com/mozilla/rust/issues/2242 is fixed.
impl of to_mustache for post {
    fn to_mustache() -> mustache::data {
        hash_from_strs([
            ("_id", self.id),
            ("title", self.title()),
            ("body", self.body())
        ]).to_mustache()
    }
}

fn render_200(req: request, mu: mustache::context, path: str,
              data: mustache::data) -> response {
    let data = alt check data {
        mustache::map(m) { m }
    };

    let template = mu.render_file(path, data);
    http_200(req, str::bytes(template))
}

fn main() {
    let ctx =
        alt zmq::init(1) {
          ok(ctx) { ctx }
          err(e) { fail e.to_str() }
        };

    let mu = mustache::context("views", ".mustache");

    let es = elasticsearch::connect_with_zmq(ctx, "tcp://localhost:9700");

    let m2 = mongrel2::connect(ctx,
        "F0D32575-2ABB-4957-BC8B-12DAC8AFF13A",
        "tcp://127.0.0.1:9998",
        "tcp://127.0.0.1:9999");

    let mre = mre::mre(m2, io::stdout());

    mre.router.add("GET", "^/$") { |req, _m|
        let posts = post::all(es);

        render_200(req, mu, "index", hash_from_strs([
            ("posts", posts.to_mustache())
        ]).to_mustache())
    }

    mre.router.add("GET", "^/posts/new$") { |req, _m|
        let post = post::post("");

        #error("%?", post.to_mustache());

        render_200(req, mu, "new", post.to_mustache())
    }

    mre.router.add("GET", "^/posts/(?<id>[-_A-Za-z0-9]+)$") { |req, m|
        alt post::find(es, m.named("id")) {
          none { http_404(req) }
          some(post) {
            render_200(req, mu, "show", post.to_mustache())
          }
        }
    }

    mre.router.add("GET", "^/posts/(?<id>[-_A-Za-z0-9]+)/edit$") { |req, m|
        alt post::find(es, m.named("id")) {
          none { http_404(req) }
          some(post) {
            render_200(req, mu, "edit", post.to_mustache())
          }
        }
    }

    mre.router.add("POST", "^/posts$") { |req, _m|
        let form = uri::decode_qs(req.body);
        let post = post::post("");

        alt form.find("title") {
          some(title) { post.set_title(title[0]) }
          none {}
        }

        alt form.find("body") {
          some(body) { post.set_body(body[0]) }
          none {}
        }

        alt post.save(es) {
          none { http_400(req) }
          some(id) { redirect(req, "/posts/" + id) }
        }
    }

    mre.router.add("POST", "^/posts/(?<id>[-_A-Za-z0-9]+)$") { |req, m|
        let id = m.named("id");
        let form = uri::decode_qs(req.body);

        alt post::find(es, id) {
          none { http_404(req) }
          some(post) {
            alt form.find("title") {
              some(title) { post.set_title(title[0]) }
              none {}
            }

            alt form.find("body") {
              some(body) { post.set_body(body[0]) }
              none {}
            }

            alt post.save(es) {
              none { http_400(req) }
              some(id) { redirect(req, "/posts/" + id) }
            }
          }
        }
    }

    mre.router.add("POST", "^/posts/(?<id>[-_A-Za-z0-9]+)/delete$") { |req, m|
        alt post::find(es, m.named("id")) {
          none { http_404(req) }
          some(post) {
            post.delete(es);

            redirect(req, "/")
          }
        }
    }

    mre.run();

    m2.term();
    ctx.term();
}
